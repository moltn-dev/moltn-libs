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

<svelte:head><title>Home</title></svelte:head>
<h1>moltn spike</h1>
<button id="counter" onclick={() => count++}>clicks: {count}</button>
<p><Link href="/users/42">User 42</Link></p>
<form onsubmit={submit}>
  <input bind:value={form.name} name="name" placeholder="name" />
  <button type="submit" disabled={form.processing}>Add</button>
</form>
<ul>
  {#each names as n (n)}<li>{n}</li>{/each}
</ul>
