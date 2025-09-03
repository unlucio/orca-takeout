<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";

  let profiles: string[] = [];
  let selected = "";

  async function loadProfiles() {
    profiles = await invoke<string[]>("list_user_filament_profiles");
    if (profiles.length && !profiles.includes(selected)) selected = profiles[0];
  }

  onMount(loadProfiles);

  async function exportProfile() {
    if (!selected) return;
    const path = await save({
      defaultPath: `${selected} profile.json`,
      title: "Export filament profile",
    });
    if (!path) return;

    await invoke("export_filament_profile", {
      start: selected,
      outputPath: path,
    });
  }
</script>

<div style="display:flex; gap:0.5rem; align-items:center;">
  <select bind:value={selected}>
    {#if profiles.length === 0}
      <option disabled selected>— no user profiles found —</option>
    {:else}
      {#each profiles as p}
        <option value={p}>{p}</option>
      {/each}
    {/if}
  </select>

  <button on:click={loadProfiles} title="Refresh">↻</button>
  <button on:click={exportProfile} disabled={!selected}>Export…</button>
</div>