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
      <Head title="Home" />
      <h1>moltn spike</h1>
      <button id="counter" onClick={() => setCount(count + 1)}>clicks: {count}</button>
      <p><Link href="/users/42">User 42</Link></p>
      <form onSubmit={submit}>
        <input value={form.data.name} onChange={(e) => form.setData('name', e.target.value)} name="name" placeholder="name" />
        <button type="submit" disabled={form.processing}>Add</button>
      </form>
      <ul>{names.map((n) => <li key={n}>{n}</li>)}</ul>
    </>
  )
}
