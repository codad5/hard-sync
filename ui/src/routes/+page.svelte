<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

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
  let loading = $state(true);
  let error = $state<string | null>(null);

  // ── Lifecycle ────────────────────────────────────────────────────────────────

  // onMount fires once when the component is first added to the DOM
  onMount(async () => {
    await loadPairs();
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
    try {
      const report = await invoke<SyncReport>("cmd_trigger_sync", { name, dryRun: false });
      lastReport = { ...lastReport, [name]: report };
    } catch (e) {
      error = String(e);
    } finally {
      syncing = { ...syncing, [name]: false };
    }
  }

  async function toggleWatcher(name: string) {
    try {
      if (statuses[name]?.running) {
        await invoke("cmd_stop_watcher", { name });
      } else {
        await invoke("cmd_start_watcher", { name });
      }
      await refreshStatuses();
    } catch (e) {
      error = String(e);
    }
  }

  async function removePair(name: string) {
    if (!confirm(`Remove pair "${name}"? This does not delete any files.`)) return;
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
    if (r.errors.length) parts.push(`${r.errors.length} errors`);
    return parts.length ? parts.join("  ·  ") : "Up to date";
  }
</script>

<div class="min-h-screen bg-[#1a1a1a] text-[#e8e8e8] p-5">
  <div class="max-w-3xl mx-auto">

    <!-- Header -->
    <header class="flex items-center justify-between mb-6">
      <h1 class="text-lg font-semibold text-white">hard-sync</h1>
      <button
        onclick={loadPairs}
        class="text-[#888] hover:text-white hover:bg-[#2a2a2a] rounded px-2 py-1 text-xl transition-colors"
        title="Refresh"
        aria-label="Refresh"
      >
        ↻
      </button>
    </header>

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
        <p class="text-sm">Run <code class="bg-[#2a2a2a] px-1.5 py-0.5 rounded text-xs">hsync init</code> in a terminal to set one up.</p>
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

            <!-- Last sync report -->
            {#if report}
              <p class="mt-2 text-xs text-green-600">{reportSummary(report)}</p>
            {/if}

            <!-- Running PID -->
            {#if status?.running && status.pid}
              <p class="mt-1 text-[11px] text-[#555]">PID {status.pid}</p>
            {/if}

          </li>
        {/each}
      </ul>
    {/if}

  </div>
</div>
