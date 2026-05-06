import { useQuery } from '@tanstack/react-query';

type Health = { status: string; version: string };

async function fetchHealth(): Promise<Health> {
  const r = await fetch('/api/v1/health');
  if (!r.ok) throw new Error(`health: ${r.status}`);
  return r.json();
}

export function App() {
  const { data, isLoading, error } = useQuery({
    queryKey: ['health'],
    queryFn: fetchHealth,
    refetchInterval: 5000,
  });

  return (
    <main className="mx-auto max-w-5xl p-8">
      <header className="flex items-baseline justify-between border-b border-zinc-800 pb-4">
        <h1 className="font-mono text-2xl tracking-tight">
          <span className="text-emerald-400">bgplg</span>
          <span className="text-zinc-500"> — looking glass</span>
        </h1>
        <span className="font-mono text-xs text-zinc-500">
          {isLoading ? 'connecting…' : error ? 'offline' : `api ${data?.version}`}
        </span>
      </header>

      <section className="mt-10 grid gap-6 md:grid-cols-2">
        <Placeholder title="Query"   note="prefix · AS# · community · AS-path regex" />
        <Placeholder title="VRFs"    note="per-router VPNv4 / VPNv6 tables" />
        <Placeholder title="SR-MPLS" note="prefix-SID · adj-SID · SR-Policy" />
        <Placeholder title="SRv6"    note="locators · End / End.DT4 / End.DT6 SIDs" />
      </section>

      <footer className="mt-16 font-mono text-xs text-zinc-600">
        M0 scaffold — see docs/PLAN.md for the full roadmap.
      </footer>
    </main>
  );
}

function Placeholder({ title, note }: { title: string; note: string }) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-5">
      <div className="font-mono text-sm text-zinc-300">{title}</div>
      <div className="mt-1 font-mono text-xs text-zinc-500">{note}</div>
    </div>
  );
}
