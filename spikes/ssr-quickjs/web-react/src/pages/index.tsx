import { useState } from 'react'
import { Head, Link, useForm } from '@inertiajs/react'

export default function Index({ names }: { names: string[] }) {
  const [count, setCount] = useState(0)
  const form = useForm({ name: '' })
  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    form.post('/names', { onSuccess: () => form.reset() })
  }
  return (
    <>
      <Head title="Accueil" />
      <h1>moltn spike</h1>
      <button id="counter" onClick={() => setCount(count + 1)}>clics : {count}</button>
      <p><Link href="/users/42">Utilisateur 42</Link></p>
      <form onSubmit={submit}>
        <input value={form.data.name} onChange={(e) => form.setData('name', e.target.value)} name="name" placeholder="nom" />
        <button type="submit" disabled={form.processing}>Ajouter</button>
      </form>
      <ul>{names.map((n) => <li key={n}>{n}</li>)}</ul>
    </>
  )
}
