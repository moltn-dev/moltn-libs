export default function Boom({ mode }: { mode: 'loop' | 'throw' }) {
  if (mode === 'loop') while (true) {}
  if (mode === 'throw') throw new Error('boom volontaire')
  return <p>jamais rendu</p>
}
