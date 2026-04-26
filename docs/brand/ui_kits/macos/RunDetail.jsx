/* global React, Icon, Capsule, SectionHeader, STATUS_META, RUNS */

const { useState } = React;

const STAGES = [
  { id: 1, name: "Capture", agent: "intake", state: "done", duration: "0:08" },
  { id: 2, name: "Plan",    agent: "planner", state: "done", duration: "1:42" },
  { id: 3, name: "Execute", agent: "implementor", state: "done", duration: "8:11" },
  { id: 4, name: "Review",  agent: "reviewer",    state: "approval", duration: "0:24" },
  { id: 5, name: "Gate",    agent: "lead",        state: "pending",  duration: "—" },
  { id: 6, name: "Deliver", agent: "deliverer",   state: "pending",  duration: "—" },
  { id: 7, name: "Seal",    agent: "reporter",    state: "pending",  duration: "—" },
];

const ARTIFACTS = [
  { kind: "json", name: "run-plan.json",        meta: "frozen · sha256:8f3a…b9", size: "12 KB", color: "#0A84FF" },
  { kind: "doc",  name: "intake-notes.md",      meta: "stage 1",                size: "2.1 KB", color: "#0A84FF" },
  { kind: "doc",  name: "plan-proposal.md",     meta: "stage 2 · accepted",     size: "8.4 KB", color: "#34C759" },
  { kind: "diff", name: "implementation.diff",  meta: "+128 / −42",             size: "7.8 KB", color: "#FF9F0A" },
  { kind: "doc",  name: "review-report.md",     meta: "awaiting approval",      size: "4.2 KB", color: "#FFD60A" },
  { kind: "json", name: "transition-log.json",  meta: "frozen",                 size: "1.7 KB", color: "#0A84FF" },
];

const KINDICON = { json:"json", doc:"doc", diff:"diff" };

const StageRow = ({ stage }) => {
  const cls = stage.state === "done" ? "done"
            : stage.state === "running" ? "running"
            : stage.state === "approval" ? "approval"
            : stage.state === "failed" ? "failed" : "";
  return (
    <div className={"fg-stage " + cls}>
      <div style={{display:"flex",justifyContent:"space-between",alignItems:"center",gap:10}}>
        <div style={{display:"flex",flexDirection:"column",gap:2}}>
          <div style={{font:"600 13px/17px var(--font-text)"}}>
            <span style={{color:"var(--label-tertiary)",fontFamily:"var(--font-mono)",fontSize:11,marginRight:6}}>{String(stage.id).padStart(2,"0")}</span>
            {stage.name}
          </div>
          <div style={{font:"var(--t-caption1)",color:"var(--label-secondary)",fontFamily:"var(--font-mono)"}}>
            {stage.agent}
          </div>
        </div>
        <div style={{display:"flex",alignItems:"center",gap:8}}>
          {stage.state === "approval" && <Capsule status="approval" size="sm" />}
          {stage.state === "running"  && <Capsule status="running"  size="sm" />}
          {stage.state === "done"     && <Capsule status="completed" size="sm" />}
          <span style={{font:"var(--t-caption1)",color:"var(--label-tertiary)",fontFamily:"var(--font-mono)",minWidth:40,textAlign:"right"}}>{stage.duration}</span>
        </div>
      </div>
    </div>
  );
};

const ArtifactRow = ({ a }) => (
  <div style={{
    display:"flex",alignItems:"center",gap:10,
    background:"var(--bg-elevated)",padding:"8px 12px",borderRadius:10,
    boxShadow:"inset 0 0 0 0.5px rgba(0,0,0,0.06)",
  }}>
    <div style={{
      width:24,height:24,borderRadius:7,
      background:`color-mix(in srgb, ${a.color} 16%, transparent)`,
      color:a.color,
      display:"grid",placeItems:"center"
    }}>
      <Icon name={KINDICON[a.kind]} size={13} color={a.color}/>
    </div>
    <div style={{display:"flex",flexDirection:"column",gap:1,flex:1,minWidth:0}}>
      <div style={{font:"600 12.5px/16px var(--font-text)",overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>{a.name}</div>
      <div style={{font:"var(--t-caption2)",color:"var(--label-secondary)",fontFamily:"var(--font-mono)",textTransform:"none",letterSpacing:0}}>{a.meta}</div>
    </div>
    <div style={{font:"var(--t-caption1)",color:"var(--label-tertiary)",fontFamily:"var(--font-mono)"}}>{a.size}</div>
  </div>
);

const RunDetail = ({ runId, onApprove, onReject }) => {
  const run = RUNS.find(r=>r.id===runId) || RUNS[0];
  const m = STATUS_META[run.status];
  const stages = STAGES.map(s => {
    if (run.status==="completed") return {...s, state: "done"};
    if (run.status==="running" && s.id <= 2) return {...s, state: "done"};
    if (run.status==="running" && s.id === 3) return {...s, state: "running"};
    if (run.status==="failed" && s.id === 3) return {...s, state: "failed"};
    if (run.status==="blocked" && s.id === 5) return {...s, state: "blocked"};
    return s;
  });

  return (
    <div style={{display:"flex",flexDirection:"column",gap:18}}>
      {/* Header */}
      <div className="fg-card">
        <div style={{display:"flex",alignItems:"flex-start",justifyContent:"space-between",gap:12}}>
          <div style={{display:"flex",flexDirection:"column",gap:4,flex:1}}>
            <div className="mono" style={{color:"var(--label-tertiary)",letterSpacing:"0.02em"}}>{run.id} · {run.workflow}</div>
            <div style={{font:"600 22px/28px var(--font-display)",letterSpacing:"-0.022em"}}>{run.title}</div>
            <div style={{display:"flex",gap:14,marginTop:4,flexWrap:"wrap",font:"var(--t-footnote)",color:"var(--label-secondary)"}}>
              <span><strong style={{color:"var(--label)"}}>Provider</strong> {run.provider}</span>
              <span><strong style={{color:"var(--label)"}}>Stage</strong> {run.stage}</span>
              <span><strong style={{color:"var(--label)"}}>Started</strong> {run.started}</span>
              <span><strong style={{color:"var(--label)"}}>Artifacts</strong> {run.artifacts}</span>
            </div>
          </div>
          <Capsule status={run.status} />
        </div>
      </div>

      {/* Approval banner */}
      {run.status==="approval" && (
        <div className="fg-approval-banner">
          <div style={{
            width:32,height:32,borderRadius:8,
            background:"color-mix(in srgb, #FFD60A 22%, transparent)",
            color:"#B8860B",display:"grid",placeItems:"center",flexShrink:0,
          }}>
            <Icon name="seal" size={18} color="#B8860B"/>
          </div>
          <div style={{display:"flex",flexDirection:"column",gap:4,flex:1}}>
            <div style={{font:"600 14px/19px var(--font-text)"}}>Reviewer submitted a proposal</div>
            <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>
              Open <span className="mono" style={{background:"var(--bg-fill-quaternary)",padding:"1px 5px",borderRadius:4}}>review-report.md</span> to inspect findings before continuing the run.
            </div>
          </div>
          <div style={{display:"flex",gap:6}}>
            <button className="fg-btn" onClick={onReject}>Reject</button>
            <button className="fg-btn success" onClick={onApprove}><Icon name="check" size={13} color="white"/> Approve</button>
          </div>
        </div>
      )}

      {run.banner && run.status==="blocked" && (
        <div style={{
          background:"color-mix(in srgb, #FF9F0A 12%, white)",
          border:"0.5px solid color-mix(in srgb, #FF9F0A 40%, transparent)",
          borderRadius:12,padding:"12px 14px",
          display:"flex",gap:12,alignItems:"flex-start"
        }}>
          <Icon name="flame" size={18} color="#FF9F0A"/>
          <div style={{flex:1}}>
            <div style={{font:"600 14px/19px var(--font-text)"}}>{run.banner}</div>
            <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>Compatibility daemon failed health check. Run is paused until provider readiness reports green.</div>
          </div>
          <button className="fg-btn">Open diagnostics</button>
        </div>
      )}

      {/* Stages */}
      <div className="fg-card">
        <SectionHeader icon="branch" title="Stages" subtitle="Frozen workflow snapshot · 7 stages" trailing={
          <button className="fg-btn ghost">Open YAML</button>
        }/>
        <div className="fg-stage-rail">
          {stages.map(s => <StageRow key={s.id} stage={s} />)}
        </div>
      </div>

      {/* Artifacts */}
      <div className="fg-card">
        <SectionHeader icon="doc" title="Artifacts" subtitle={`${ARTIFACTS.length} durable outputs · stored on disk`}/>
        <div style={{display:"flex",flexDirection:"column",gap:6}}>
          {ARTIFACTS.map(a => <ArtifactRow key={a.name} a={a} />)}
        </div>
      </div>
    </div>
  );
};

window.RunDetail = RunDetail;
window.STAGES = STAGES;
window.ARTIFACTS = ARTIFACTS;
