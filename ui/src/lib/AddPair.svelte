<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  // Props — $props() is Svelte 5's way of declaring component inputs
  let { onAdded, onCancel }: {
    onAdded: () => void;
    onCancel: () => void;
  } = $props();

  // Form state
  let name   = $state("");
  let base   = $state("");
  let target = $state("");
  let source = $state<"base" | "target">("base");
  let error  = $state<string | null>(null);
  let saving = $state(false);

  // Example paths in the style of the user's OS (placeholder text only)
  const EXAMPLES: Record<string, { base: string; target: string }> = {
    windows: { base: "C:\\Users\\you\\projects", target: "E:\\backup" },
    macos:   { base: "/Users/you/projects",          target: "/Volumes/MY_USB/backup" },
    linux:   { base: "/home/you/projects",           target: "/media/you/MY_USB/backup" },
  };
  let examples = $state(EXAMPLES.linux);
  invoke<string>("cmd_platform")
    .then((os) => { examples = EXAMPLES[os] ?? EXAMPLES.linux; })
    .catch(() => {});

  async function pickFolder(field: "base" | "target") {
    // open() shows the OS native folder picker
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      if (field === "base")   base   = selected;
      if (field === "target") target = selected;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onCancel();
  }

  async function submit() {
    error = null;
    if (!name.trim())   { error = "Name is required";        return; }
    if (!base.trim())   { error = "Base path is required";   return; }
    if (!target.trim()) { error = "Target path is required"; return; }

    saving = true;
    try {
      await invoke("cmd_add_pair", { name: name.trim(), base, target, source });
      onAdded();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- Sheet: dims the window and drops down from the toolbar, like an NSWindow sheet -->
<div class="fixed inset-0 z-20 bg-black/25 flex justify-center items-start pt-[52px]" role="presentation">
  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="add-pair-title"
    class="sheet w-[460px] max-w-[calc(100vw-32px)] rounded-b-xl bg-group shadow-[0_12px_40px_rgb(0_0_0/0.28),0_0_0_0.5px_rgb(0_0_0/0.2)] px-5 pt-4 pb-4"
  >
  <form onsubmit={(e) => { e.preventDefault(); submit(); }}>
    <h2 id="add-pair-title" class="text-[13px] font-semibold text-label mb-3">New Sync Pair</h2>

    {#if error}
      <div role="alert" class="mb-3 rounded-md bg-danger-bg px-2.5 py-1.5 text-[12px] text-danger selectable">{error}</div>
    {/if}

    <!-- Right-aligned labels, fields in a column: the classic macOS form layout -->
    <div class="grid grid-cols-[88px_1fr] items-center gap-x-3 gap-y-2.5">
      <label for="pair-name" class="text-right text-label">Name:</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="pair-name" class="field w-full" type="text" bind:value={name} placeholder="usb-backup" autofocus />

      <label for="pair-base" class="text-right text-label">Folder:</label>
      <div class="flex gap-2 min-w-0">
        <input id="pair-base" class="field flex-1 min-w-0" type="text" bind:value={base} placeholder={examples.base} />
        <button type="button" class="btn" onclick={() => pickFolder("base")}>Choose…</button>
      </div>

      <label for="pair-target" class="text-right text-label">Other side:</label>
      <div class="flex gap-2 min-w-0">
        <input id="pair-target" class="field flex-1 min-w-0" type="text" bind:value={target} placeholder={examples.target} />
        <button type="button" class="btn" onclick={() => pickFolder("target")}>Choose…</button>
      </div>

      <span class="text-right text-label">Copy:</span>
      <!-- Segmented control -->
      <div role="radiogroup" aria-label="Copy direction" class="inline-flex w-fit rounded-md bg-window p-0.5 shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.12)]">
        <button type="button" role="radio" aria-checked={source === "base"} onclick={() => (source = "base")}
          class="px-3 h-[20px] rounded-[5px] text-[12px] {source === 'base' ? 'bg-control text-label shadow-[0_0.5px_1.5px_rgb(0_0_0/0.2)]' : 'text-secondary'}">
          Folder → Other side
        </button>
        <button type="button" role="radio" aria-checked={source === "target"} onclick={() => (source = "target")}
          class="px-3 h-[20px] rounded-[5px] text-[12px] {source === 'target' ? 'bg-control text-label shadow-[0_0.5px_1.5px_rgb(0_0_0/0.2)]' : 'text-secondary'}">
          Other side → Folder
        </button>
      </div>
      <span></span>
      <p class="text-[11px] text-secondary -mt-1">
        The other side follows the source: new and changed files are copied over, removed files go to its <code class="font-mono">.hard-sync-trash</code>.
      </p>
    </div>

    <div class="mt-4 flex justify-end gap-2">
      <button type="button" class="btn min-w-[72px] justify-center" onclick={onCancel}>Cancel</button>
      <button type="submit" class="btn btn-primary min-w-[72px] justify-center" disabled={saving}>
        {saving ? "Adding…" : "Add"}
      </button>
    </div>
  </form>
  </div>
</div>

<style>
  .sheet { animation: sheet-in 180ms ease-out; }
  @keyframes sheet-in {
    from { transform: translateY(-12px); opacity: 0; }
    to   { transform: none; opacity: 1; }
  }
</style>
