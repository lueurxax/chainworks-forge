/* global React, Icon, Capsule, SectionHeader */

const Inspector = ({ runId }) => (
  <div style={{display:"flex",flexDirection:"column",gap:18}}>
    <SectionHeader icon="shield" title="Run snapshot" subtitle="Frozen at run start" />
    <div className="fg-panel" style={{padding:14,display:"flex",flexDirection:"column",gap:10}}>
      {[
        ["Run ID", runId],
        ["Workflow", "proposal-loop-live@v3"],
        ["Catalog", "agents.yaml · sha256:7c4d…2a"],
        ["Provider", "Claude Code"],
        ["Model", "claude-sonnet-4-5"],
        ["Runtime", "ACP"],
        ["Started", "2026-04-26T12:18:33Z"],
      ].map(([k,v])=>(
        <div key={k} style={{display:"flex",gap:8,justifyContent:"space-between",alignItems:"baseline"}}>
          <div style={{font:"var(--t-caption2)",color:"var(--label-tertiary)",textTransform:"uppercase",letterSpacing:"0.04em",fontWeight:600}}>{k}</div>
          <div className="mono" style={{color:"var(--label)",textAlign:"right",overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap",maxWidth:"60%"}}>{v}</div>
        </div>
      ))}
    </div>

    <SectionHeader icon="seal" iconColor="#B8860B" title="Approval gate" subtitle="Reviewer · stage 4" />
    <div className="fg-panel" style={{padding:14,display:"flex",flexDirection:"column",gap:8}}>
      <div style={{font:"var(--t-footnote)",color:"var(--label-secondary)"}}>The reviewer submitted a proposal at stage 4. The run pauses until the operator continues, rejects, or holds.</div>
      <div style={{display:"flex",gap:6,marginTop:6}}>
        <button className="fg-btn success" style={{flex:1}}><Icon name="check" size={13} color="white"/> Approve</button>
        <button className="fg-btn" style={{flex:1}}>Reject</button>
      </div>
      <button className="fg-btn ghost" style={{justifyContent:"flex-start",padding:"4px 0"}}>Hold for later</button>
    </div>

    <SectionHeader icon="history" title="Recovery" subtitle="Resume from last sealed checkpoint" />
    <div className="fg-panel" style={{padding:14,display:"flex",flexDirection:"column",gap:8}}>
      <div style={{display:"flex",alignItems:"center",gap:8}}>
        <Icon name="check" size={14} color="var(--status-success)"/>
        <span style={{font:"var(--t-callout)"}}>Last checkpoint · stage 3</span>
      </div>
      <div style={{display:"flex",alignItems:"center",gap:8}}>
        <Icon name="check" size={14} color="var(--status-success)"/>
        <span style={{font:"var(--t-callout)"}}>Artifact discovery bounded</span>
      </div>
      <button className="fg-btn" style={{marginTop:4}}>Open recovery sheet</button>
    </div>
  </div>
);

window.Inspector = Inspector;
