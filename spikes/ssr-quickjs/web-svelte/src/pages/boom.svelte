<script lang="ts">
  let { mode }: { mode: 'loop' | 'throw' } = $props()
  if (mode === 'loop') while (true) {}
  if (mode === 'throw') throw new Error('boom volontaire')
</script>

<p>jamais rendu</p>
