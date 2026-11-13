<script lang="ts">
  import { Link } from '@inertiajs/svelte'

  let { user }: { user: { id: number; name: string } } = $props()
</script>

<svelte:head><title>{user.name}</title></svelte:head>
<h1>{user.name} (#{user.id})</h1>
<Link href="/">Back</Link>
