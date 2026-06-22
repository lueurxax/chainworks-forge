/* global React, Icon, Capsule, SectionHeader, STATUS_META */

const RUNS = [
  { id: "RUN-0248", title: "Bound stage-settlement reads to projection truth",
    status: "approval", workflow: "full-mvp-live", provider: "Claude Code",
    stage: "4 · review", started: "2m ago", progress: 0.55,
    artifacts: 12, approvals: 1 },
  { id: "RUN-0247", title: "Wire daemon supervisor to packaged-mode crash budget",
    status: "running", workflow: "full-mvp-live", provider: "Codex",
    stage: "2 · plan", started: "11m ago", progress: 0.32,
    artifacts: 6, approvals: 0 },
  { id: "RUN-0246", title: "Dynamic cycle addition · proposal-020",
    status: "blocked", workflow: "proposal-to-release", provider: "Claude Code",
    stage: "5 · gate", started: "34m ago", progress: 0.7,
    artifacts: 18, approvals: 0, banner: "Goose runtime not ready" },
  { id: "RUN-0245", title: "Approved-host current-head proof gate",
    status: "completed", workflow: "full-mvp-live", provider: "Gemini",
    stage: "7 · sealed", started: "1h ago", progress: 1,
    artifacts: 24, approvals: 2 },
  { id: "RUN-0244", title: "Frozen provider/model provenance truth in run snapshot",
    status: "completed", workflow: "full-mvp-live", provider: "Claude Code",
    stage: "7 · sealed", started: "3h ago", progress: 1,
    artifacts: 17, approvals: 1 },
  { id: "RUN-0243", title: "ACP fallback diagnostics export bundle",
    status: "failed", workflow: "proposal-to-release", provider: "Codex",
    stage: "3 · execute", started: "5h ago", progress: 0.42,
    artifacts: 9, approvals: 0 },
];

const RunRow = ({ run, active, onClick }) => {
  const m = STATUS_META[run.status];
  return (
    <div onClick={onClick}
      style={{
        background: active ? "color-mix(in srgb, var(--tint) 8%, var(--bg-elevated))" : "var(--bg-elevated)",
        borderRadius: 12,
        padding: "12px 14px",
        boxShadow: active
          ? "inset 0 0 0 1.5px var(--tint), 0 1px 2px rgba(0,0,0,0.04)"
          : "0 1px 2px rgba(0,0,0,0.04), inset 0 0 0 0.5px rgba(0,0,0,0.06)",
        cursor: "pointer",
        display: "flex", flexDirection: "column", gap: 8,
        transition: "all 0.15s cubic-bezier(.32,.72,0,1)",
      }}>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"flex-start",gap:8}}>
        <div style={{display:"flex",flexDirection:"column",gap:3,minWidth:0,flex:1}}>
          <div style={{font:"600 11px/13px var(--font-text)",color:"var(--label-tertiary)",fontFamily:"var(--font-mono)",letterSpacing:"0.02em"}}>
            {run.id} · {run.workflow}
          </div>
          <div style={{font:"600 14px/19px var(--font-text)",color:"var(--label)",overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>
            {run.title}
          </div>
        </div>
        <Capsule status={run.status} />
      </div>

      <div style={{display:"flex",gap:14,alignItems:"center",font:"var(--t-footnote)",color:"var(--label-secondary)"}}>
        <span style={{display:"flex",alignItems:"center",gap:4}}><Icon name="branch" size={12} color="var(--label-tertiary)"/> {run.stage}</span>
        <span style={{display:"flex",alignItems:"center",gap:4}}><Icon name="bolt" size={12} color="var(--label-tertiary)"/> {run.provider}</span>
        <span style={{display:"flex",alignItems:"center",gap:4}}><Icon name="clock" size={12} color="var(--label-tertiary)"/> {run.started}</span>
        <span style={{flex:1}}/>
        <span style={{display:"flex",alignItems:"center",gap:4}}><Icon name="doc" size={12} color="var(--label-tertiary)"/> {run.artifacts}</span>
        {run.approvals>0 && <span style={{display:"flex",alignItems:"center",gap:4,color:"#B8860B"}}><Icon name="seal" size={12} color="#B8860B"/> {run.approvals}</span>}
      </div>

      <div style={{height:3,background:"var(--bg-fill-tertiary)",borderRadius:999,overflow:"hidden"}}>
        <div style={{
          height:"100%", width: `${run.progress*100}%`,
          background: m.color, borderRadius: 999,
          transition:"width 0.4s cubic-bezier(.32,.72,0,1)"
        }}/>
      </div>
    </div>
  );
};

const RunsHome = ({ filter, selectedId, onSelect }) => {
  const filtered = filter === "All"
    ? RUNS
    : RUNS.filter(r => {
        if (filter==="Active")   return ["running","approval"].includes(r.status);
        if (filter==="Blocked")  return r.status==="blocked";
        if (filter==="Completed")return r.status==="completed";
        return true;
      });
  const active = filtered.filter(r=>["running","approval"].includes(r.status));
  const others = filtered.filter(r=>!["running","approval"].includes(r.status));

  return (
    <div style={{display:"flex",flexDirection:"column",gap:18}}>
      {active.length>0 && (
        <section>
          <SectionHeader icon="bolt" iconColor="#0A84FF" title="Active runs" subtitle={`${active.length} run${active.length===1?"":"s"} executing or awaiting approval`} />
          <div style={{display:"flex",flexDirection:"column",gap:8}}>
            {active.map(r=> <RunRow key={r.id} run={r} active={r.id===selectedId} onClick={()=>onSelect(r.id)} />)}
          </div>
        </section>
      )}
      {others.length>0 && (
        <section>
          <SectionHeader icon="history" iconColor="var(--label-secondary)" title="Recent" subtitle="Completed, blocked, and failed runs" />
          <div style={{display:"flex",flexDirection:"column",gap:8}}>
            {others.map(r=> <RunRow key={r.id} run={r} active={r.id===selectedId} onClick={()=>onSelect(r.id)} />)}
          </div>
        </section>
      )}
    </div>
  );
};

window.RUNS = RUNS;
window.RunsHome = RunsHome;
