<script lang="ts">
  import { Link, useForm } from '@inertiajs/svelte'

  let { names }: { names: string[] } = $props()
  let count = $state(0)
  const form = useForm({ name: '' })
  function submit(e: SubmitEvent) {
    e.preventDefault()
    form.post('/names', { onSuccess: () => form.reset() })
  }
</script>

<svelte:head><title>Accueil</title></svelte:head>
<h1>moltn spike</h1>
<button id="counter" onclick={() => count++}>clics : {count}</button>
<p><Link href="/users/42">Utilisateur 42</Link></p>
<form onsubmit={submit}>
  <input bind:value={form.name} name="name" placeholder="nom" />
  <button type="submit" disabled={form.processing}>Ajouter</button>
</form>
<ul>
  {#each names as n (n)}<li>{n}</li>{/each}
</ul>
