/* global React */
// Forge UI primitives — capsules, icons, section header, etc.

const { useState, useEffect, useMemo, useRef } = React;

// Lucide-style stroke icons (trimmed inline set)
const Icon = ({ name, size = 16, color = "currentColor", strokeWidth = 1.7 }) => {
  const paths = {
    "play":      "M8 5v14l11-7z",
    "bolt":      "M13 2L3 14h7l-1 8 10-12h-7l1-8z",
    "check":     "M5 12l4 4 10-10",
    "x":         "M6 6l12 12M18 6L6 18",
    "pause":     "M9 5v14M15 5v14",
    "clock":     "M12 7v5l3 2",
    "hourglass": "M6 3h12M6 21h12M8 3v3a4 4 0 008 0V3M8 21v-3a4 4 0 018-0v3",
    "ban":       "M5 5l14 14",
    "seal":      "M12 2l2.4 2.6 3.5-.5.5 3.5L21 10l-2.6 2.4.5 3.5-3.5.5L12 19l-2.4-2.6-3.5.5-.5-3.5L3 10l2.6-2.4-.5-3.5 3.5-.5z",
    "search":    "M11 4a7 7 0 100 14 7 7 0 000-14zm5 11l5 5",
    "plus":      "M12 5v14M5 12h14",
    "filter":    "M3 5h18l-7 9v6l-4-2v-4z",
    "more":      "M5 12h.01M12 12h.01M19 12h.01",
    "chevron-r": "M9 6l6 6-6 6",
    "chevron-d": "M6 9l6 6 6-6",
    "settings":  "M12 8v0a4 4 0 010 8 4 4 0 010-8zM19.4 13a7.5 7.5 0 000-2l2-1.5-2-3.4-2.4.8a7.5 7.5 0 00-1.7-1L15 3.5h-4l-.4 2.4a7.5 7.5 0 00-1.7 1l-2.4-.8-2 3.4L6.6 11a7.5 7.5 0 000 2l-2 1.5 2 3.4 2.4-.8a7.5 7.5 0 001.7 1L11 20.5h4l.4-2.4a7.5 7.5 0 001.7-1l2.4.8 2-3.4z",
    "runs":      "M3 12a9 9 0 1118 0 9 9 0 01-18 0zM12 7v5l3 2",
    "ideas":     "M9 21h6M10 17h4M12 3a6 6 0 016 6c0 3-2 5-3 6H9c-1-1-3-3-3-6a6 6 0 016-6z",
    "approvals": "M9 12l2 2 5-5M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
    "catalog":   "M3 5h18M3 12h18M3 19h18",
    "workflow":  "M4 6h6v4H4zM14 6h6v4h-6zM4 14h6v4H4zM14 14h6v4h-6zM10 8h4M10 16h4",
    "pilot":     "M2 12l3-3 3 3-3 3zM12 2l3 3-3 3-3-3zM22 12l-3 3-3-3 3-3zM12 22l-3-3 3-3 3 3z",
    "doc":       "M14 3H6a2 2 0 00-2 2v14a2 2 0 002 2h12a2 2 0 002-2V9zM14 3v6h6",
    "diff":      "M8 4l-5 5 5 5M16 20l5-5-5-5M9 9l6 6",
    "json":      "M8 6c-3 0-3 6 0 6s0 6 0 6M16 6c3 0 3 6 0 6s0 6 0 6",
    "shield":    "M12 2l8 4v6c0 5-4 9-8 10-4-1-8-5-8-10V6z",
    "flame":     "M12 2c4 4 6 7 6 11a6 6 0 11-12 0c0-2 1-4 3-5 0 2 1 4 3 4-1-3 0-7 0-10z",
    "branch":    "M6 3v18M6 9c0 3 3 4 6 4s6 1 6 4M18 3v6",
    "history":   "M3 12a9 9 0 1118 0 9 9 0 01-18 0zM3 12h4l2-3 4 6 2-3h4M12 7v5l3 2",
    "bell":      "M6 8a6 6 0 1112 0c0 7 3 9 3 9H3s3-2 3-9zM10 21a2 2 0 004 0",
    "send":      "M22 2L11 13M22 2l-7 20-4-9-9-4z",
  };
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none"
      stroke={color} strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round"
      style={{ flexShrink: 0, display: "block" }}>
      <path d={paths[name] || ""} />
    </svg>
  );
};

// Status capsule
const STATUS_META = {
  running:   { label: "Running",           color: "#0A84FF", icon: "bolt" },
  approval:  { label: "Awaiting approval", color: "#B8860B", icon: "seal" },
  completed: { label: "Completed",         color: "#1C8A3D", icon: "check" },
  failed:    { label: "Failed",            color: "#D70015", icon: "x" },
  blocked:   { label: "Blocked",           color: "#C25A00", icon: "pause" },
  pending:   { label: "Pending",           color: "#8E8E93", icon: "clock" },
  ready:     { label: "Ready",             color: "#8E8E93", icon: "clock" },
  cancelled: { label: "Cancelled",         color: "#8E8E93", icon: "ban" },
  cancelling:{ label: "Cancelling",        color: "#8E8E93", icon: "hourglass" },
};

const Capsule = ({ status, label, color, icon, size = "md" }) => {
  const m = status ? STATUS_META[status] : { label, color, icon };
  const c = m.color;
  const small = size === "sm";
  return (
    <span className="fg-cap" style={{
      color: c,
      background: `color-mix(in srgb, ${c} 14%, transparent)`,
      padding: small ? "2px 7px" : "3px 8px",
      fontSize: small ? "10px" : "11px",
    }}>
      <span className="dot" style={{ width: small ? 5 : 6, height: small ? 5 : 6 }} />
      {m.label}
    </span>
  );
};

// Section header (ForgeSectionHeader pattern)
const SectionHeader = ({ icon, iconColor = "var(--tint)", title, subtitle, trailing }) => (
  <div className="fg-section-header">
    {icon && <span style={{ marginTop: 2 }}><Icon name={icon} color={iconColor} size={16} /></span>}
    <div className="titles">
      <div className="t">{title}</div>
      {subtitle && <div className="s">{subtitle}</div>}
    </div>
    <div style={{ flex: 1 }} />
    {trailing}
  </div>
);

// Sidebar nav item
const NavItem = ({ icon, label, count, active, onClick }) => (
  <div className={"fg-nav-item" + (active ? " active" : "")} onClick={onClick}>
    <Icon name={icon} size={15} color={active ? "white" : "var(--label-secondary)"} />
    <span>{label}</span>
    {count != null && <span className="count">{count}</span>}
  </div>
);

Object.assign(window, { Icon, Capsule, SectionHeader, NavItem, STATUS_META });
