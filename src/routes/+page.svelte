<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";

  let name = $state("");
  let greetMsg = $state("");

  async function saveHello() {
    const path = await save({
      defaultPath: "hello.txt",
      title: "Save text file",
    });
    if (!path) return; // user cancelled
    await invoke("save_text", { path, contents: "hello from tauri" });
  }
</script>

<button on:click={saveHello}>Save a file…</button>