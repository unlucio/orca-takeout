<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";

  let profile = "Broken PETG HF";

  async function exportProfile() {
    // build JSON string (if you want to preview)
    // const json = await invoke<string>("build_filament_profile", { start: profile });

    // or write directly
    const path = await save({
      defaultPath: `${profile} profile.json`,
      title: "Export filament profile",
    });
    if (!path) return;

    await invoke<string>("export_filament_profile", {
      start: profile,
      outputPath: path,
    });
  }
</script>

<input bind:value={profile} placeholder="Profile name (e.g., Broken PETG HF)" />
<button on:click={exportProfile}>Export profile…</button>