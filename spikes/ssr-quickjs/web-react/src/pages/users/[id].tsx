import { Head, Link } from '@inertiajs/react'

export default function Show({ user }: { user: { id: number; name: string } }) {
  return (
    <>
      <Head title={user.name} />
      <h1>{user.name} (#{user.id})</h1>
      <Link href="/">Retour</Link>
    </>
  )
}
