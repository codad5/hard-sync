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

<div class="min-h-screen bg-[#1a1a1a] text-[#e8e8e8] p-5">
  <div class="max-w-3xl mx-auto">

    <!-- Header -->
    <header class="flex items-center justify-between mb-6">
      <h1 class="text-lg font-semibold text-white">hard-sync</h1>
      <div class="flex gap-2 items-center">
        <button
          onclick={() => (showAddForm = !showAddForm)}
          class="text-sm px-3 py-1.5 rounded bg-blue-700 text-white hover:bg-blue-600 transition-colors font-medium"
        >
          + Add pair
        </button>
        <button
          onclick={loadPairs}
          class="text-[#888] hover:text-white hover:bg-[#2a2a2a] rounded px-2 py-1 text-xl transition-colors"
          title="Refresh"
          aria-label="Refresh"
        >
          ↻
        </button>
      </div>
    </header>

    <!-- Add pair form (shown inline when + button is clicked) -->
    {#if showAddForm}
      <AddPair
        onAdded={() => { showAddForm = false; loadPairs(); }}
        onCancel={() => (showAddForm = false)}
      />
    {/if}

    <!-- Error banner -->
    {#if error}
      <div class="flex justify-between items-center bg-[#3a1a1a] border border-[#5a2a2a] text-red-400 rounded-md px-4 py-2.5 mb-4 text-sm">
        <span>{error}</span>
        <button onclick={() => (error = null)} class="ml-4 hover:text-white">✕</button>
      </div>
    {/if}

    <!-- Loading -->
    {#if loading}
      <p class="text-center text-[#666] py-16">Loading pairs…</p>

    <!-- Empty state -->
    {:else if pairs.length === 0}
      <div class="text-center py-16 text-[#666]">
        <p class="text-base mb-2">No sync pairs configured.</p>
        <p class="text-sm">Click <span class="text-white">+ Add pair</span> to set one up, or run <code class="bg-[#2a2a2a] px-1.5 py-0.5 rounded text-xs">hsync init</code> in a terminal.</p>
      </div>

    <!-- Pair list -->
    {:else}
      <!-- {#each} iterates an array, the (pair.name) is a key for efficient DOM updates -->
      <ul class="flex flex-col gap-3">
        {#each pairs as pair (pair.name)}
          {@const status = statuses[pair.name]}
          {@const report = lastReport[pair.name]}
          {@const isSyncing = syncing[pair.name]}

          <li class="bg-[#242424] border border-[#333] rounded-lg px-4 py-3.5">

            <!-- Card header: name + badges + action buttons -->
            <div class="flex flex-wrap items-center justify-between gap-3 mb-2">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="font-semibold text-white">{pair.name}</span>
                <!-- Status badge -->
                <span class="text-xs px-2 py-0.5 rounded-full font-medium
                  {status?.running
                    ? 'bg-green-950 text-green-400 border border-green-900'
                    : 'bg-[#2a2a2a] text-[#888] border border-[#3a3a3a]'}">
                  {status?.running ? "watching" : "idle"}
                </span>
                {#if pair.drive_id}
                  <span class="text-xs px-2 py-0.5 rounded-full font-medium bg-blue-950 text-blue-400 border border-blue-900">
                    drive pair
                  </span>
                {/if}
              </div>

              <!-- Action buttons -->
              <div class="flex items-center gap-1.5">
                <button
                  onclick={() => triggerSync(pair.name)}
                  disabled={isSyncing}
                  class="text-xs px-2.5 py-1 rounded bg-[#2a2a2a] border border-[#3a3a3a] text-[#ccc] hover:bg-[#333] hover:text-white disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {isSyncing ? "Syncing…" : "Sync now"}
                </button>
                <button
                  onclick={() => toggleWatcher(pair.name)}
                  class="text-xs px-2.5 py-1 rounded font-medium transition-colors
                    {status?.running
                      ? 'bg-red-950 text-red-300 border border-red-900 hover:bg-red-900'
                      : 'bg-blue-700 text-white hover:bg-blue-600'}"
                >
                  {status?.running ? "Stop" : "Watch"}
                </button>
                <button
                  onclick={() => removePair(pair.name)}
                  class="text-xs px-2 py-1 rounded text-[#666] border border-[#333] hover:bg-[#2a2a2a] hover:text-[#aaa] transition-colors"
                  title="Remove pair"
                >
                  ✕
                </button>
              </div>
            </div>

            <!-- Paths row -->
            <div class="flex items-center gap-2 text-xs text-[#888] overflow-hidden">
              <span class="truncate max-w-[260px] {pair.source === 'base' ? 'text-blue-400' : ''}">{pair.base}</span>
              <span class="shrink-0 text-[#555]">{pair.source === "base" ? "→" : "←"}</span>
              <span class="truncate max-w-[260px] {pair.source === 'target' ? 'text-blue-400' : ''}">{pair.target}</span>
            </div>

            <!-- Last sync: failed as a whole -->
            {#if syncFailure[pair.name]}
              <div role="alert" class="mt-2 text-xs text-red-300 bg-[#3a1a1a] border border-[#5a2a2a] rounded px-3 py-2">
                <span class="font-medium text-red-400">Sync failed:</span> {syncFailure[pair.name]}
              </div>
            {/if}

            <!-- Last sync report -->
            {#if report}
              <p class="mt-2 text-xs {report.errors.length ? 'text-red-400' : 'text-green-600'}">{reportSummary(report)}</p>
              {#if report.errors.length}
                <details class="mt-1 text-xs">
                  <summary class="cursor-pointer text-red-400 hover:text-red-300 select-none">
                    Show {report.errors.length} {report.errors.length === 1 ? "error" : "errors"}
                  </summary>
                  <ul class="mt-1.5 max-h-48 overflow-y-auto bg-[#1e1e1e] border border-[#3a2a2a] rounded divide-y divide-[#2a2a2a]">
                    {#each report.errors as err}
                      <li class="px-3 py-1.5">
                        <div class="text-[#ccc] break-all">{err.path}</div>
                        <div class="text-red-400/90">{err.message}</div>
                      </li>
                    {/each}
                  </ul>
                </details>
              {/if}
            {/if}

            <!-- Live watcher status -->
            {#if status?.running}
              {@const activity = watchActivity[pair.name]}
              <p class="mt-1 text-[11px] text-[#777]">
                {#if status.owner === "external"}
                  Watched from a terminal (<code>hsync watch</code>, PID {status.pid}).
                {:else if activity}
                  {activity.message.split("\n")[0]}
                {:else}
                  Starting watcher…
                {/if}
              </p>
            {/if}

            <!-- Watcher log (app and terminal watchers both write it) -->
            <button
              onclick={() => toggleLog(pair.name)}
              class="mt-1.5 text-[11px] text-[#666] hover:text-[#aaa]"
            >
              {logs[pair.name] ? "Hide log" : "Show log"}
            </button>
            {#if logs[pair.name]}
              {@const lines = logs[pair.name] ?? []}
              <pre class="mt-1 max-h-56 overflow-y-auto bg-[#1a1a1a] border border-[#333] rounded p-2 text-[11px] leading-relaxed whitespace-pre-wrap break-all">{#if lines.length === 0}<span class="text-[#666]">No watcher activity yet.</span>{:else}{#each lines as line}<span class={/(error|failed|! )/i.test(line) ? "text-red-400" : "text-[#999]"}>{line}</span>{"\n"}{/each}{/if}</pre>
            {/if}

          </li>
        {/each}
      </ul>
    {/if}

  </div>
</div>
