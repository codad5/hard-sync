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

  async function pickFolder(field: "base" | "target") {
    // open() shows the OS native folder picker
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      if (field === "base")   base   = selected;
      if (field === "target") target = selected;
    }
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

<div class="bg-[#1e1e1e] border border-[#3a3a3a] rounded-lg p-5 mb-4">
  <h2 class="text-white font-semibold text-base mb-4">Add sync pair</h2>

  {#if error}
    <div class="text-red-400 text-sm bg-red-950 border border-red-900 rounded px-3 py-2 mb-4">
      {error}
    </div>
  {/if}

  <!-- Name -->
  <div class="mb-3">
    <label class="block text-xs text-[#888] mb-1" for="pair-name">Name</label>
    <input
      id="pair-name"
      type="text"
      bind:value={name}
      placeholder="e.g. usb-backup"
      class="w-full bg-[#2a2a2a] border border-[#3a3a3a] rounded px-3 py-2 text-sm text-white placeholder-[#555] focus:outline-none focus:border-blue-600"
    />
  </div>

  <!-- Base path -->
  <div class="mb-3">
    <label class="block text-xs text-[#888] mb-1" for="pair-base">Base path (local folder)</label>
    <div class="flex gap-2">
      <input
        id="pair-base"
        type="text"
        bind:value={base}
        placeholder="C:\Users\you\projects"
        class="flex-1 bg-[#2a2a2a] border border-[#3a3a3a] rounded px-3 py-2 text-sm text-white placeholder-[#555] focus:outline-none focus:border-blue-600"
      />
      <button
        onclick={() => pickFolder("base")}
        class="px-3 py-2 bg-[#2a2a2a] border border-[#3a3a3a] rounded text-sm text-[#aaa] hover:bg-[#333] hover:text-white transition-colors"
      >
        Browse
      </button>
    </div>
  </div>

  <!-- Target path -->
  <div class="mb-3">
    <label class="block text-xs text-[#888] mb-1" for="pair-target">Target path (drive or other folder)</label>
    <div class="flex gap-2">
      <input
        id="pair-target"
        type="text"
        bind:value={target}
        placeholder="E:\backup"
        class="flex-1 bg-[#2a2a2a] border border-[#3a3a3a] rounded px-3 py-2 text-sm text-white placeholder-[#555] focus:outline-none focus:border-blue-600"
      />
      <button
        onclick={() => pickFolder("target")}
        class="px-3 py-2 bg-[#2a2a2a] border border-[#3a3a3a] rounded text-sm text-[#aaa] hover:bg-[#333] hover:text-white transition-colors"
      >
        Browse
      </button>
    </div>
  </div>

  <!-- Source side toggle -->
  <div class="mb-5">
    <label class="block text-xs text-[#888] mb-2">Source of truth (files flow from here)</label>
    <div class="flex gap-2">
      <button
        onclick={() => source = "base"}
        class="flex-1 py-2 rounded text-sm font-medium transition-colors
          {source === 'base'
            ? 'bg-blue-700 text-white'
            : 'bg-[#2a2a2a] border border-[#3a3a3a] text-[#888] hover:text-white'}"
      >
        Base → Target
      </button>
      <button
        onclick={() => source = "target"}
        class="flex-1 py-2 rounded text-sm font-medium transition-colors
          {source === 'target'
            ? 'bg-blue-700 text-white'
            : 'bg-[#2a2a2a] border border-[#3a3a3a] text-[#888] hover:text-white'}"
      >
        Target → Base
      </button>
    </div>
  </div>

  <!-- Actions -->
  <div class="flex gap-2 justify-end">
    <button
      onclick={onCancel}
      class="px-4 py-2 rounded text-sm text-[#888] border border-[#333] hover:bg-[#2a2a2a] hover:text-white transition-colors"
    >
      Cancel
    </button>
    <button
      onclick={submit}
      disabled={saving}
      class="px-4 py-2 rounded text-sm font-medium bg-blue-700 text-white hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
    >
      {saving ? "Adding…" : "Add pair"}
    </button>
  </div>
</div>
