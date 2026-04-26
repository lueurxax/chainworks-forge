/* global React, Icon, Capsule, SectionHeader, NavItem, STATUS_META */

const { useState } = React;

const Sidebar = ({ active, setActive, counts }) => (
  <aside className="fg-sidebar">
    <div className="brand">
      <svg width="28" height="28" viewBox="0 0 200 200" style={{display:"block",borderRadius:8}}>
        <defs>
          <linearGradient id="sb-bg" x1="0" y1="0" x2="0" y2="1"><stop stopColor="#1B2738"/><stop offset="1" stopColor="#04080D"/></linearGradient>
          <linearGradient id="sb-orbit" x1="20" y1="150" x2="180" y2="60"><stop stopColor="#A3AFC0"/><stop offset="1" stopColor="#475365"/></linearGradient>
        </defs>
        <rect width="200" height="200" rx="44" fill="url(#sb-bg)"/>
        <path d="M30 150 C 60 110, 100 80, 170 60" stroke="url(#sb-orbit)" strokeWidth="6" strokeLinecap="round" fill="none" opacity="0.85"/>
        <path d="M55 105 c14 -8 30 -10 50 -6 c-8 12 -22 18 -36 16 c-6 -2 -10 -6 -14 -10z" fill="#FAFBFD"/>
        <path d="M95 96 c4 -3 10 -5 14 -4 c-2 4 -6 6 -10 8 z" fill="#F59A2B"/>
      </svg>
      <div style={{display:"flex",flexDirection:"column",gap:1}}>
        <div className="name">Chainworks</div>
        <div className="sub">Forge</div>
      </div>
    </div>

    <div className="fg-section-label">Workspace</div>
    <NavItem icon="runs"      label="Runs"           count={counts.runs}      active={active==="runs"}      onClick={()=>setActive("runs")} />
    <NavItem icon="ideas"     label="Ideas"          count={counts.ideas}     active={active==="ideas"}     onClick={()=>setActive("ideas")} />
    <NavItem icon="approvals" label="Approvals"      count={counts.approvals} active={active==="approvals"} onClick={()=>setActive("approvals")} />

    <div className="fg-section-label">Catalog</div>
    <NavItem icon="catalog"  label="Agent catalog"     active={active==="catalog"}  onClick={()=>setActive("catalog")} />
    <NavItem icon="workflow" label="Workflow inspector" active={active==="workflow"} onClick={()=>setActive("workflow")} />
    <NavItem icon="pilot"    label="Pilot readiness"    active={active==="pilot"}    onClick={()=>setActive("pilot")} />

    <div style={{flex:1}} />
    <NavItem icon="settings" label="Settings" active={active==="settings"} onClick={()=>setActive("settings")} />
  </aside>
);

const Toolbar = ({ title, segments, segActive, setSegActive, action }) => (
  <div className="fg-toolbar">
    <div className="title">{title}</div>
    {segments && (
      <div className="fg-segmented" style={{marginLeft:14}}>
        {segments.map(s=>(
          <div key={s} className={s===segActive?"active":""} onClick={()=>setSegActive(s)}>{s}</div>
        ))}
      </div>
    )}
    <div className="spacer" />
    <button className="fg-btn" style={{background:"transparent",padding:"6px 8px"}}>
      <Icon name="search" size={14} color="var(--label-secondary)" />
    </button>
    <button className="fg-btn" style={{background:"transparent",padding:"6px 8px"}}>
      <Icon name="bell" size={14} color="var(--label-secondary)" />
    </button>
    {action && <button className="fg-btn primary"><Icon name="plus" size={13} color="white"/> {action}</button>}
  </div>
);

window.Sidebar = Sidebar;
window.Toolbar = Toolbar;
