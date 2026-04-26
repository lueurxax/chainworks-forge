/* global React, Sidebar, Toolbar, RunsHome, RunDetail, Inspector, Capsule, SectionHeader, Icon, RUNS */

const { useState } = React;

function App() {
  const [active, setActive] = useState("runs");
  const [filter, setFilter] = useState("Active");
  const [selectedId, setSelectedId] = useState("RUN-0248");

  const counts = {
    runs: RUNS.filter(r => ["running","approval","blocked"].includes(r.status)).length,
    ideas: 7,
    approvals: RUNS.filter(r => r.status==="approval").length,
  };

  return (
    <div style={{display:"flex",height:"100vh",overflow:"hidden"}}>
      <Sidebar active={active} setActive={setActive} counts={counts} />
      <div className="fg-main">
        <Toolbar
          title={
            active === "runs" ? "Runs" :
            active === "ideas" ? "Ideas" :
            active === "approvals" ? "Approvals" :
            active === "catalog" ? "Agent catalog" :
            active === "workflow" ? "Workflow inspector" :
            active === "pilot" ? "Pilot readiness" : "Settings"
          }
          segments={active === "runs" ? ["All","Active","Blocked","Completed"] : null}
          segActive={filter} setSegActive={setFilter}
          action={active === "ideas" ? "Capture idea" : active === "runs" ? "New run" : null}
        />
        <div style={{display:"flex",flex:1,overflow:"hidden"}}>
          <div className="fg-content" style={{flex:1}}>
            {active === "runs" && (
              <RunsHome filter={filter} selectedId={selectedId} onSelect={setSelectedId}/>
            )}
            {active === "ideas" && <IdeasView/>}
            {active === "approvals" && <ApprovalsView/>}
            {active === "catalog" && <CatalogView/>}
            {active === "workflow" && <WorkflowView/>}
            {active === "pilot" && <PilotView/>}
            {active === "settings" && <SettingsView/>}
          </div>
          {active === "runs" && (
            <aside className="fg-inspector">
              <RunDetail runId={selectedId}/>
            </aside>
          )}
        </div>
      </div>
    </div>
  );
}

const IdeasView = () => (
  <div style={{display:"flex",flexDirection:"column",gap:18}}>
    <div className="fg-card">
      <SectionHeader icon="ideas" title="Capture an idea" subtitle="A unit of engineering work · optionally tied to files or a workspace"/>
      <textarea className="fg-idea-input" rows="3" placeholder="What should the run accomplish?"
        defaultValue="Refactor stage settlement so artifact discovery is bounded, and ensure projection truth is read consistently across operator surfaces."/>
      <div style={{display:"flex",gap:8,marginTop:10,alignItems:"center"}}>
        <button className="fg-btn"><Icon name="catalog" size={13}/> Workflow…</button>
        <button className="fg-btn"><Icon name="branch" size={13}/> Workspace…</button>
        <div style={{flex:1}}/>
        <button className="fg-btn primary"><Icon name="send" size={13} color="white"/> Compile run</button>
      </div>
    </div>
    <SectionHeader icon="history" title="Recent ideas"/>
    {[
      ["Bound stage-settlement reads to projection truth","Compiled · RUN-0248"],
      ["Wire daemon supervisor to packaged-mode crash budget","Compiled · RUN-0247"],
      ["Approved-host current-head proof gate","Sealed · RUN-0245"],
      ["Frozen provider/model provenance truth","Sealed · RUN-0244"],
    ].map(([t,m])=>(
      <div key={t} className="fg-card" style={{display:"flex",alignItems:"center",gap:12}}>
        <Icon name="ideas" size={18} color="var(--label-tertiary)"/>
        <div style={{flex:1}}>
          <div style={{font:"600 14px/19px var(--font-text)"}}>{t}</div>
          <div className="mono" style={{color:"var(--label-secondary)",fontSize:11}}>{m}</div>
        </div>
        <Icon name="chevron-r" size={14} color="var(--label-tertiary)"/>
      </div>
    ))}
  </div>
);

const ApprovalsView = () => (
  <div style={{display:"flex",flexDirection:"column",gap:18}}>
    <SectionHeader icon="approvals" iconColor="#B8860B" title="Pending approvals" subtitle="Operator decisions blocking run continuation"/>
    {[
      ["RUN-0248","review-report.md","Reviewer flagged 3 medium-severity findings on stage settlement.","2m"],
    ].map(([id,a,desc,t])=>(
      <div key={id} className="fg-approval-banner">
        <div style={{width:32,height:32,borderRadius:8,background:"color-mix(in srgb, #FFD60A 22%, transparent)",color:"#B8860B",display:"grid",placeItems:"center",flexShrink:0}}>
          <Icon name="seal" size={18} color="#B8860B"/>
        </div>
        <div style={{flex:1}}>
          <div style={{font:"600 14px/19px var(--font-text)"}}>{id} · {a}</div>
          <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>{desc}</div>
        </div>
        <span style={{font:"var(--t-caption1)",color:"var(--label-tertiary)",fontFamily:"var(--font-mono)"}}>{t}</span>
        <button className="fg-btn">Reject</button>
        <button className="fg-btn success"><Icon name="check" size={13} color="white"/> Approve</button>
      </div>
    ))}
    <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)",textAlign:"center",padding:"12px 0"}}>1 pending · approvals are diagnostic-only in P031</div>
  </div>
);

const CatalogView = () => (
  <div style={{display:"flex",flexDirection:"column",gap:12}}>
    <SectionHeader icon="catalog" title="Agent catalog" subtitle="Resolved from agents.yaml · 6 agents bound"/>
    {[
      ["intake","Captures ideas into structured run inputs","Codex","gpt-5"],
      ["planner","Produces frozen plan proposals","Claude Code","claude-sonnet-4-5"],
      ["implementor","Executes the plan against the worktree","Claude Code","claude-sonnet-4-5"],
      ["reviewer","Reads diffs · returns review report","Gemini","gemini-2.5-pro"],
      ["lead","Mediates workflow conflict & rejection","Codex","gpt-5"],
      ["reporter","Seals the run report at completion","Claude Code","claude-haiku-4-5"],
    ].map(([n,d,p,m])=>(
      <div key={n} className="fg-card" style={{display:"flex",alignItems:"center",gap:12}}>
        <div style={{width:32,height:32,borderRadius:8,background:"var(--bg-fill-tertiary)",display:"grid",placeItems:"center"}}>
          <span className="mono" style={{fontSize:13,fontWeight:700}}>{n[0]}</span>
        </div>
        <div style={{flex:1}}>
          <div className="mono" style={{font:"600 13px/16px var(--font-mono)"}}>{n}</div>
          <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>{d}</div>
        </div>
        <div style={{display:"flex",flexDirection:"column",alignItems:"flex-end",gap:1}}>
          <div style={{font:"var(--t-caption1)",fontWeight:600}}>{p}</div>
          <div className="mono" style={{fontSize:10,color:"var(--label-tertiary)"}}>{m}</div>
        </div>
      </div>
    ))}
  </div>
);

const WorkflowView = () => (
  <div className="fg-card">
    <SectionHeader icon="workflow" title="proposal-loop-live@v3" subtitle="Frozen workflow snapshot · 7 stages · 2 approval gates"/>
    <pre style={{
      background:"#0E1623",color:"#E5EAF2",borderRadius:10,padding:"14px 16px",
      font:"12px/18px var(--font-mono)",overflow:"auto",margin:0,
    }}>{`stages:
  - id: capture
    agent: intake
  - id: plan
    agent: planner
    transitions: [accepted, rejected]
  - id: execute
    agent: implementor
  - id: review
    agent: reviewer
    approval_gate: true        # ← stage 4
  - id: gate
    agent: lead
    approval_gate: true        # ← stage 5
  - id: deliver
    agent: deliverer
  - id: seal
    agent: reporter`}</pre>
  </div>
);

const PilotView = () => (
  <div style={{display:"flex",flexDirection:"column",gap:12}}>
    <SectionHeader icon="pilot" title="Pilot readiness" subtitle="Sign-off support for the local pilot host"/>
    {[
      ["Local daemon health","green","Daemon is supervised, PID lock held."],
      ["Provider provenance","green","Codex · Claude Code · Gemini all return frozen versions."],
      ["Approved-host current head","amber","Last UI smoke gate ran 2h ago. Re-run before sign-off."],
      ["Diagnostics export","green","Bundle path resolved; size 4.2 MB."],
    ].map(([k,s,d])=>{
      const dotColor = s === "green" ? "var(--success)"
                      : s === "amber" ? "var(--warning)"
                      : "var(--danger)";
      const status = s === "green" ? "completed" : s === "amber" ? "approval" : "failed";
      return (
        <div key={k} className="fg-card" style={{display:"flex",alignItems:"center",gap:12}}>
          {/* Fixed-width status dot keeps title flush-left across all rows. */}
          <div style={{
            width:10,height:10,borderRadius:"50%",
            background:dotColor,flexShrink:0,
            boxShadow:`0 0 0 3px color-mix(in oklab, ${dotColor} 18%, transparent)`,
          }}/>
          <div style={{flex:1,minWidth:0}}>
            <div style={{font:"600 14px/19px var(--font-text)"}}>{k}</div>
            <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>{d}</div>
          </div>
          <Capsule status={status} size="sm"/>
        </div>
      );
    })}
  </div>
);

const SettingsView = () => (
  <div style={{display:"flex",flexDirection:"column",gap:18}}>
    <SectionHeader icon="settings" title="Providers" subtitle="Bind ACP-capable runtimes for live execution"/>
    {[
      ["Claude Code","ACP","claude-sonnet-4-5","Connected","completed"],
      ["Codex","ACP","gpt-5","Connected","completed"],
      ["Gemini","ACP","gemini-2.5-pro","Connected","completed"],
      ["Goose","compat","legacy","Daemon not ready","blocked"],
    ].map(([p,t,m,s,st])=>(
      <div key={p} className="fg-card" style={{display:"flex",alignItems:"center",gap:14}}>
        <div style={{width:36,height:36,borderRadius:9,background:"var(--bg-fill-tertiary)",display:"grid",placeItems:"center"}}>
          <span className="mono" style={{fontSize:14,fontWeight:700}}>{p[0]}</span>
        </div>
        <div style={{flex:1}}>
          <div style={{font:"600 14px/19px var(--font-text)"}}>{p}</div>
          <div className="mono" style={{font:"var(--t-caption1)",color:"var(--label-secondary)"}}>{t} · {m}</div>
        </div>
        <Capsule status={st} size="sm"/>
        <button className="fg-btn">Configure</button>
      </div>
    ))}
  </div>
);

const root = ReactDOM.createRoot(document.getElementById("root"));
root.render(<App/>);
