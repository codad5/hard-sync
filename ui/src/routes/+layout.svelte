<script lang="ts">
  import "../app.css";
  import { invoke } from "@tauri-apps/api/core";
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();

  // Expose the OS as <html data-platform="macos|windows|linux"> so styles can
  // adapt (e.g. leave room for macOS traffic-light buttons in the title bar).
  invoke<string>("cmd_platform")
    .then((os) => document.documentElement.setAttribute("data-platform", os))
    .catch(() => {});
</script>

{@render children()}
