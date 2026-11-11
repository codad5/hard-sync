<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import AddPair from "$lib/AddPair.svelte";

  // ── Types (mirror Rust structs) ─────────────────────────────────────────────

  interface DriveId {
    label: string | null;
    uuid: string | null;
  }

  interface PairConfig {
    name: string;
    base: string;
    target: string;
    source: string;
    drive_id: DriveId | null;
    delete_behavior: string;
    created_at: string;
  }

  interface WatcherStatus {
    name: string;
    running: boolean;
    pid: number | null;
    /** "app" = watched by this app, "external" = by `hsync watch` in a terminal */
    owner: "app" | "external" | null;
  }

  /** Payload of the "watch-event" event emitted by the Rust watcher. */
  interface WatchUpdate {
    name: string;
    kind: "watching" | "drive_detected" | "drive_removed" | "sync_started" | "sync_completed" | "sync_error";
    message: string;
    report: SyncReport | null;
  }

  interface SyncReport {
    copied: number;
    updated: number;
    trashed: number;
    deleted: number;
    skipped: number;
    ignored: number;
    errors: { path: string; message: string }[];
    ops: { rel_path: string; outcome: string }[];
  }

  // ── State ────────────────────────────────────────────────────────────────────

  // $state() is Svelte 5's reactive variable — re-renders when assigned
  let pairs = $state<PairConfig[]>([]);
  let statuses = $state<Record<string, WatcherStatus>>({});
  let syncing = $state<Record<string, boolean>>({});
  let lastReport = $state<Record<string, SyncReport>>({});
  // Why the last sync of a pair failed as a whole (e.g. target drive is read-only).
  // Shown on that pair's card, next to the thing it is about.
  let syncFailure = $state<Record<string, string | null>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let showAddForm = $state(false);
  // Latest live watcher message per pair ("Drive removed. Waiting for it.")
  let watchActivity = $state<Record<string, WatchUpdate>>({});
  // Watcher log lines per pair, loaded when "Show log" is opened
  let logs = $state<Record<string, string[] | null>>({});

  // ── Lifecycle ────────────────────────────────────────────────────────────────

  // onMount fires once when the component is first added to the DOM
  onMount(() => {
    loadPairs();

    // Live events from watchers running in this app
    const unlistenWatch = listen<WatchUpdate>("watch-event", ({ payload: u }) => {
      watchActivity = { ...watchActivity, [u.name]: u };
      if (u.kind === "sync_started") {
        syncFailure = { ...syncFailure, [u.name]: null };
      } else if (u.kind === "sync_completed" && u.report) {
        lastReport = { ...lastReport, [u.name]: u.report };
      } else if (u.kind === "sync_error") {
        syncFailure = { ...syncFailure, [u.name]: u.message.replace(/^Sync failed: /, "") };
      }
      if (logs[u.name]) loadLog(u.name);
    });
    // Tray "Stop all watchers"
    const unlistenStopped = listen("watchers-changed", () => refreshStatuses());
    // Terminal watchers can start/stop at any time: refresh when the window is focused
    const onFocus = () => refreshStatuses();
    window.addEventListener("focus", onFocus);

    return () => {
      unlistenWatch.then((f) => f());
      unlistenStopped.then((f) => f());
      window.removeEventListener("focus", onFocus);
    };
  });

  // ── Functions ────────────────────────────────────────────────────────────────

  async function loadPairs() {
    loading = true;
    error = null;
    try {
      // invoke() calls a #[tauri::command] on the Rust side by name
      pairs = await invoke<PairConfig[]>("cmd_list_pairs");
      await refreshStatuses();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function refreshStatuses() {
    const results = await Promise.all(
      pairs.map((p) => invoke<WatcherStatus>("cmd_watcher_status", { name: p.name }))
    );
    const map: Record<string, WatcherStatus> = {};
    for (const s of results) map[s.name] = s;
    statuses = map;
  }

  async function triggerSync(name: string) {
    syncing = { ...syncing, [name]: true };
    syncFailure = { ...syncFailure, [name]: null };
    try {
      const report = await invoke<SyncReport>("cmd_trigger_sync", { name, dryRun: false });
      lastReport = { ...lastReport, [name]: report };
    } catch (e) {
      const { [name]: _, ...rest } = lastReport;
      lastReport = rest;
      syncFailure = { ...syncFailure, [name]: String(e) };
    } finally {
      syncing = { ...syncing, [name]: false };
    }
  }

  async function toggleWatcher(name: string) {
    try {
      if (statuses[name]?.running) {
        await invoke("cmd_stop_watcher", { name });
        const { [name]: _, ...rest } = watchActivity;
        watchActivity = rest;
      } else {
        await invoke("cmd_start_watcher", { name });
      }
    } catch (e) {
      error = String(e);
    }
    await refreshStatuses();
  }

  async function loadLog(name: string) {
    try {
      logs = { ...logs, [name]: await invoke<string[]>("cmd_watcher_log", { name, lines: 200 }) };
    } catch (e) {
      error = String(e);
    }
  }

  function toggleLog(name: string) {
    if (logs[name]) {
      logs = { ...logs, [name]: null };
    } else {
      loadLog(name);
    }
  }

  async function removePair(name: string) {
    // Must be the dialog plugin's ask(), awaited. Inside the Tauri webview the
    // browser's confirm() doesn't block, so the pair was removed before the
    // user had answered.
    const yes = await ask(`Remove pair "${name}"? This does not delete any files.`, {
      title: "Remove pair",
      kind: "warning",
      okLabel: "Remove",
      cancelLabel: "Cancel",
    });
    if (!yes) return;
    try {
      await invoke("cmd_remove_pair", { name });
      await loadPairs();
    } catch (e) {
      error = String(e);
    }
  }

  function reportSummary(r: SyncReport): string {
    const parts = [];
    if (r.copied)        parts.push(`${r.copied} copied`);
    if (r.updated)       parts.push(`${r.updated} updated`);
    if (r.trashed)       parts.push(`${r.trashed} trashed`);
    if (r.skipped)       parts.push(`${r.skipped} skipped`);
    if (r.errors.length) parts.push(`${r.errors.length} ${r.errors.length === 1 ? "error" : "errors"}`);
    return parts.length ? parts.join("  ·  ") : "Up to date";
  }
</script>

<!--
  Layout follows macOS System Settings: a unified toolbar the window can be
  dragged by, then grouped "inset" lists with hairline separators.
-->
<div class="min-h-screen flex flex-col">

  <!-- Toolbar (drag region). On macOS the traffic lights sit at its left. -->
  <header
    data-tauri-drag-region
    class="toolbar sticky top-0 z-10 h-[52px] shrink-0 flex items-center gap-2 pr-3 bg-window/85 backdrop-blur-xl border-b border-separator"
  >
    <h1 data-tauri-drag-region class="text-[13px] font-semibold text-label">hard-sync</h1>
    <div data-tauri-drag-region class="flex-1 h-full"></div>
    <button class="icon-btn" onclick={loadPairs} title="Refresh" aria-label="Refresh">
      <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9"/><path d="M13.5 2.5v3h-3"/></svg>
    </button>
    <button class="icon-btn" onclick={() => (showAddForm = true)} title="Add pair" aria-label="Add pair">
      <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true"><path d="M8 3v10M3 8h10"/></svg>
    </button>
  </header>

  <main class="flex-1 w-full max-w-[680px] mx-auto px-5 py-5">

    <!-- Page-level error (loading pairs, watcher start/stop) -->
    {#if error}
      <div role="alert" class="mb-4 flex items-start gap-2 rounded-lg bg-danger-bg px-3 py-2 text-danger">
        <span class="flex-1 selectable">{error}</span>
        <button class="icon-btn h-5 w-5 text-danger" onclick={() => (error = null)} aria-label="Dismiss">✕</button>
      </div>
    {/if}

    {#if loading}
      <p class="py-20 text-center text-secondary">Loading…</p>

    {:else if pairs.length === 0}
      <div class="py-20 flex flex-col items-center text-center gap-2">
        <div class="text-tertiary" aria-hidden="true">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M20 12a8 8 0 0 1-13.7 5.6M4 12a8 8 0 0 1 13.7-5.6"/><path d="M18 3v3.6h-3.6M6 21v-3.6h3.6"/></svg>
        </div>
        <p class="text-[15px] font-semibold text-label">No sync pairs</p>
        <p class="text-secondary max-w-[300px]">A pair keeps two folders in sync — for example a project folder and a backup on a USB drive.</p>
        <button class="btn btn-primary mt-2" onclick={() => (showAddForm = true)}>Add Pair…</button>
      </div>

    {:else}
      <h2 class="px-2.5 pb-1.5 text-[11px] font-semibold uppercase tracking-wide text-secondary">Sync pairs</h2>

      <ul class="rounded-[10px] bg-group shadow-[0_0_0_0.5px_rgb(0_0_0/0.12)] overflow-hidden">
        {#each pairs as pair, i (pair.name)}
          {@const status = statuses[pair.name]}
          {@const report = lastReport[pair.name]}
          {@const failure = syncFailure[pair.name]}
          {@const isSyncing = syncing[pair.name]}
          {@const activity = watchActivity[pair.name]}
          {@const hasProblem = !!failure || !!report?.errors.length}
          {@const from = pair.source === "base" ? pair.base : pair.target}
          {@const to = pair.source === "base" ? pair.target : pair.base}

          <li class="px-3 py-2.5 {i > 0 ? 'border-t border-separator' : ''}">
            <div class="flex items-center gap-3">

              <!-- Kind icon: removable drive or folder -->
              <div class="shrink-0 w-7 h-7 rounded-md flex items-center justify-center text-white {pair.drive_id ? 'bg-[#ff9500]' : 'bg-accent'}" aria-hidden="true">
                {#if pair.drive_id}
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><rect x="2" y="5" width="12" height="7" rx="1.5"/><path d="M4.5 9.5h2" stroke-linecap="round"/></svg>
                {:else}
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h3l1.5 1.5h4.5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z"/></svg>
                {/if}
              </div>

              <!-- Name, status, paths -->
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="font-semibold text-label truncate">{pair.name}</span>
                  <span class="inline-flex items-center gap-1 text-[11px] text-secondary shrink-0">
                    <span class="w-1.5 h-1.5 rounded-full {hasProblem ? 'bg-danger' : status?.running ? 'bg-ok' : 'bg-tertiary'}" aria-hidden="true"></span>
                    {hasProblem ? "Problem" : isSyncing ? "Syncing…" : status?.running ? "Watching" : "Idle"}
                  </span>
                </div>
                <div class="mt-0.5 flex items-center gap-1 text-[11px] text-secondary min-w-0" title="{from} → {to}">
                  <span class="truncate selectable">{from}</span>
                  <span class="shrink-0 text-tertiary" aria-label="copies to">→</span>
                  <span class="truncate selectable">{to}</span>
                </div>
              </div>

              <!-- Actions -->
              <div class="shrink-0 flex items-center gap-2">
                <button class="btn" onclick={() => triggerSync(pair.name)} disabled={isSyncing}>
                  {isSyncing ? "Syncing…" : "Sync"}
                </button>

                <!-- Watch: macOS-style switch -->
                <label class="inline-flex items-center gap-1.5 text-[11px] text-secondary" title="Sync automatically when files change or the drive is plugged in">
                  Watch
                  <button
                    role="switch"
                    aria-checked={!!status?.running}
                    aria-label="Watch {pair.name}"
                    onclick={() => toggleWatcher(pair.name)}
                    class="relative w-[26px] h-[15px] rounded-full transition-colors {status?.running ? 'bg-accent' : 'bg-tertiary'}"
                  >
                    <span class="absolute top-[1.5px] left-[1.5px] w-3 h-3 rounded-full bg-white shadow transition-transform {status?.running ? 'translate-x-[11px]' : ''}"></span>
                  </button>
                </label>

                <button class="icon-btn" onclick={() => removePair(pair.name)} title="Remove pair" aria-label="Remove {pair.name}">
                  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5"/></svg>
                </button>
              </div>
            </div>

            <!-- Details, aligned with the text column -->
            <div class="pl-10">
              {#if status?.running}
                <p class="mt-1.5 text-[11px] text-secondary">
                  {#if status.owner === "external"}
                    Watched from a terminal (<code class="font-mono">hsync watch</code>, PID {status.pid})
                  {:else if activity}
                    {activity.message.split("\n")[0]}
                  {:else}
                    Starting…
                  {/if}
                </p>
              {/if}

              {#if failure}
                <div role="alert" class="mt-2 rounded-md bg-danger-bg px-2.5 py-1.5 text-[12px] text-danger selectable">
                  <span class="font-semibold">Sync failed.</span> {failure}
                </div>
              {/if}

              {#if report}
                <p class="mt-1.5 text-[11px] {report.errors.length ? 'text-danger' : 'text-secondary'}">
                  Last sync: {reportSummary(report)}
                </p>
                {#if report.errors.length}
                  <details class="mt-1 text-[12px]">
                    <summary class="cursor-default text-accent select-none">
                      Show {report.errors.length} {report.errors.length === 1 ? "error" : "errors"}
                    </summary>
                    <ul class="mt-1.5 max-h-48 overflow-y-auto rounded-md border border-separator divide-y divide-separator selectable">
                      {#each report.errors as err}
                        <li class="px-2.5 py-1.5">
                          <div class="text-label break-all">{err.path}</div>
                          <div class="text-danger">{err.message}</div>
                        </li>
                      {/each}
                    </ul>
                  </details>
                {/if}
              {/if}

              <button class="mt-1 text-[11px] text-accent hover:underline" onclick={() => toggleLog(pair.name)}>
                {logs[pair.name] ? "Hide Log" : "Show Log"}
              </button>
              {#if logs[pair.name]}
                {@const lines = logs[pair.name] ?? []}
                <pre class="mt-1 max-h-56 overflow-y-auto rounded-md bg-window border border-separator p-2 font-mono text-[11px] leading-relaxed whitespace-pre-wrap break-all selectable">{#if lines.length === 0}<span class="text-secondary">No watcher activity yet.</span>{:else}{#each lines as line}<span class={/(error|failed|! )/i.test(line) ? "text-danger" : "text-secondary"}>{line}</span>{"\n"}{/each}{/if}</pre>
              {/if}
            </div>
          </li>
        {/each}
      </ul>

      <p class="px-2.5 pt-2 text-[11px] text-secondary">
        Files copy in the direction of the arrow. Removed files go to <code class="font-mono">.hard-sync-trash</code> on the receiving side.
      </p>
    {/if}
  </main>
</div>

<!-- Add pair: macOS-style sheet -->
{#if showAddForm}
  <AddPair
    onAdded={() => { showAddForm = false; loadPairs(); }}
    onCancel={() => (showAddForm = false)}
  />
{/if}

<style>
  /* Leave room for the macOS traffic-light buttons (overlay title bar). */
  :global(html[data-platform="macos"]) .toolbar { padding-left: 80px; }
  :global(html:not([data-platform="macos"])) .toolbar { padding-left: 16px; }
  /* Windows/Linux draw their own title bar with the app name — don't repeat it. */
  :global(html:not([data-platform="macos"])) .toolbar h1 { visibility: hidden; }
</style>
