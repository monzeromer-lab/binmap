/* @ds-bundle: {"format":4,"namespace":"SilverKeyLodestoneDesignSystem_2b7558","components":[{"name":"AcpAgentCard","sourcePath":"components/agents/AcpAgentCard.jsx"},{"name":"ApiKeyField","sourcePath":"components/agents/ApiKeyField.jsx"},{"name":"ModelPicker","sourcePath":"components/agents/ModelPicker.jsx"},{"name":"ProviderCard","sourcePath":"components/agents/ProviderCard.jsx"},{"name":"ConfidenceBadge","sourcePath":"components/analysis/ConfidenceBadge.jsx"},{"name":"EvidenceItem","sourcePath":"components/analysis/EvidenceItem.jsx"},{"name":"FindingCard","sourcePath":"components/analysis/FindingCard.jsx"},{"name":"GateResult","sourcePath":"components/analysis/GateResult.jsx"},{"name":"ProposalBlock","sourcePath":"components/analysis/ProposalBlock.jsx"},{"name":"ProvenanceBadge","sourcePath":"components/analysis/ProvenanceBadge.jsx"},{"name":"StackFrameList","sourcePath":"components/analysis/StackFrameList.jsx"},{"name":"TranscriptStep","sourcePath":"components/analysis/TranscriptStep.jsx"},{"name":"TrustTierControl","sourcePath":"components/analysis/TrustTierControl.jsx"},{"name":"Flamegraph","sourcePath":"components/charts/Flamegraph.jsx"},{"name":"ParetoScatter","sourcePath":"components/charts/ParetoScatter.jsx"},{"name":"Treemap","sourcePath":"components/charts/Treemap.jsx"},{"name":"CommandPalette","sourcePath":"components/chrome/CommandPalette.jsx"},{"name":"EmptyState","sourcePath":"components/chrome/EmptyState.jsx"},{"name":"NavRail","sourcePath":"components/chrome/NavRail.jsx"},{"name":"StatusBar","sourcePath":"components/chrome/StatusBar.jsx"},{"name":"TitleBar","sourcePath":"components/chrome/TitleBar.jsx"},{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Checkbox","sourcePath":"components/core/Checkbox.jsx"},{"name":"Dialog","sourcePath":"components/core/Dialog.jsx"},{"name":"Icon","sourcePath":"components/core/Icon.jsx"},{"name":"IconButton","sourcePath":"components/core/IconButton.jsx"},{"name":"Input","sourcePath":"components/core/Input.jsx"},{"name":"Panel","sourcePath":"components/core/Panel.jsx"},{"name":"SegmentedControl","sourcePath":"components/core/SegmentedControl.jsx"},{"name":"Select","sourcePath":"components/core/Select.jsx"},{"name":"Switch","sourcePath":"components/core/Switch.jsx"},{"name":"Tabs","sourcePath":"components/core/Tabs.jsx"},{"name":"Tag","sourcePath":"components/core/Tag.jsx"},{"name":"Tooltip","sourcePath":"components/core/Tooltip.jsx"},{"name":"CodeBlock","sourcePath":"components/data/CodeBlock.jsx"},{"name":"DataTable","sourcePath":"components/data/DataTable.jsx"},{"name":"DeltaValue","sourcePath":"components/data/DeltaValue.jsx"},{"name":"MetricStat","sourcePath":"components/data/MetricStat.jsx"},{"name":"ProgressBar","sourcePath":"components/data/ProgressBar.jsx"}],"sourceHashes":{"components/agents/AcpAgentCard.jsx":"b34b7d0d48f2","components/agents/ApiKeyField.jsx":"461ef6fa55d1","components/agents/ModelPicker.jsx":"6aec83ab2c80","components/agents/ProviderCard.jsx":"9f4218a255a8","components/analysis/ConfidenceBadge.jsx":"f879daecda86","components/analysis/EvidenceItem.jsx":"773f52b8f764","components/analysis/FindingCard.jsx":"cfc47ea5f884","components/analysis/GateResult.jsx":"816940fa29d1","components/analysis/ProposalBlock.jsx":"3cb6f8a71f3b","components/analysis/ProvenanceBadge.jsx":"78bde26e0242","components/analysis/StackFrameList.jsx":"eae3f97b1b8f","components/analysis/TranscriptStep.jsx":"3f3087824547","components/analysis/TrustTierControl.jsx":"164b9f04896a","components/charts/Flamegraph.jsx":"62e44ab1333f","components/charts/ParetoScatter.jsx":"0acd83d5f30c","components/charts/Treemap.jsx":"7f97b545e65d","components/chrome/CommandPalette.jsx":"ae5dff95514f","components/chrome/EmptyState.jsx":"bfe026e281cc","components/chrome/NavRail.jsx":"ec63cf90b21e","components/chrome/StatusBar.jsx":"cac3114f3dc2","components/chrome/TitleBar.jsx":"63cfb5d2b9a7","components/core/Badge.jsx":"1c4974931114","components/core/Button.jsx":"9df5b70e0852","components/core/Checkbox.jsx":"ad4614f47b19","components/core/Dialog.jsx":"25ba6e30d7f9","components/core/Icon.jsx":"c6b4c7360f3b","components/core/IconButton.jsx":"3e92305bc297","components/core/Input.jsx":"07da44a90490","components/core/Panel.jsx":"c93e46d1d844","components/core/SegmentedControl.jsx":"fc5d55cf0eb6","components/core/Select.jsx":"267cf32af90f","components/core/Switch.jsx":"90f7109219fc","components/core/Tabs.jsx":"30113658d36e","components/core/Tag.jsx":"03d902250e7b","components/core/Tooltip.jsx":"f3ce9303a7d0","components/data/CodeBlock.jsx":"3afe77c35d42","components/data/DataTable.jsx":"28e2603908e2","components/data/DeltaValue.jsx":"7ff5152187b5","components/data/MetricStat.jsx":"f2428164d10d","components/data/ProgressBar.jsx":"866d21965445","ui_kits/binmap-cli/Sessions.jsx":"667bebfbdedb","ui_kits/binmap-cli/Terminal.jsx":"3e756fbcaff9","ui_kits/binmap-desktop/AgentSetup.jsx":"02e94d2c300e","ui_kits/binmap-desktop/AgentTranscript.jsx":"a0287fc5c9cd","ui_kits/binmap-desktop/AppFrame.jsx":"1001b21623b4","ui_kits/binmap-desktop/CrashAnalysis.jsx":"4a41c6853811","ui_kits/binmap-desktop/Inspector.jsx":"375cc98f9e7d","ui_kits/binmap-desktop/PerfLab.jsx":"5fca69755eec","ui_kits/binmap-desktop/ProfileLab.jsx":"eac44c022985","ui_kits/binmap-desktop/ProjectView.jsx":"ea21ad038dc0","ui_kits/binmap-desktop/SizeExplorer.jsx":"257db2a24d66","ui_kits/binmap-desktop/data.js":"c9062135d310","ui_kits/binmap-desktop/model-catalog.js":"9bd718842531"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.SilverKeyLodestoneDesignSystem_2b7558 = window.SilverKeyLodestoneDesignSystem_2b7558 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/analysis/ConfidenceBadge.jsx
try { (() => {
const LEVELS = {
  certain: {
    label: "Certain",
    color: "var(--confidence-certain)",
    dashed: false,
    note: "Directly measured"
  },
  high: {
    label: "High",
    color: "var(--confidence-high)",
    dashed: false,
    note: "Deterministic derivation"
  },
  probable: {
    label: "Probable",
    color: "var(--confidence-probable)",
    dashed: true,
    note: "Model inference, strong evidence"
  },
  speculative: {
    label: "Speculative",
    color: "var(--confidence-speculative)",
    dashed: true,
    note: "Model inference, weak evidence"
  }
};
function ConfidenceBadge({
  level = "certain",
  showNote = false,
  style
}) {
  const l = LEVELS[level] || LEVELS.certain;
  return /*#__PURE__*/React.createElement("span", {
    title: l.note,
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-6)",
      padding: "1px 6px",
      border: "1px " + (l.dashed ? "dashed" : "solid") + " " + l.color,
      borderRadius: "var(--radius-5)",
      color: l.color,
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      whiteSpace: "nowrap",
      ...style
    }
  }, l.label, showNote ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      letterSpacing: 0,
      textTransform: "none",
      color: "var(--text-muted)"
    }
  }, l.note) : null);
}
Object.assign(__ds_scope, { ConfidenceBadge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/ConfidenceBadge.jsx", error: String((e && e.message) || e) }); }

// components/analysis/ProvenanceBadge.jsx
try { (() => {
/** DESIGN §7.1: ● measured (solid), ◈ derived (half-filled), ◆ inferred (outlined, distinct hue).
    The marker glyph and the colour are both load-bearing — never one without the other. */
const KINDS = {
  measured: {
    glyph: "\u25CF",
    label: "Measured",
    color: "var(--prov-measured)",
    bg: "var(--prov-measured-bg)"
  },
  derived: {
    glyph: "\u25C8",
    label: "Derived",
    color: "var(--prov-derived)",
    bg: "var(--prov-derived-bg)"
  },
  inferred: {
    glyph: "\u25C6",
    label: "Inferred",
    color: "var(--prov-inferred)",
    bg: "var(--prov-inferred-bg)"
  }
};
function ProvenanceBadge({
  kind = "measured",
  label,
  detail,
  variant = "chip",
  style
}) {
  const k = KINDS[kind] || KINDS.measured;
  if (variant === "marker") {
    return /*#__PURE__*/React.createElement("span", {
      title: detail || k.label,
      style: {
        color: k.color,
        fontSize: 10,
        lineHeight: 1,
        ...style
      }
    }, k.glyph);
  }
  return /*#__PURE__*/React.createElement("span", {
    title: detail,
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-4)",
      padding: "1px 6px 1px 5px",
      background: variant === "outline" ? "transparent" : k.bg,
      color: k.color,
      border: "1px solid " + k.color + "44",
      borderRadius: "var(--radius-2)",
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      whiteSpace: "nowrap",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: 9,
      lineHeight: 1
    }
  }, k.glyph), label || k.label);
}
Object.assign(__ds_scope, { ProvenanceBadge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/ProvenanceBadge.jsx", error: String((e && e.message) || e) }); }

// components/charts/Flamegraph.jsx
try { (() => {
/** Inlining-aware flamegraph. Frames marked inline are hatched so a logical frame
    is never mistaken for a physical one. */
function Flamegraph({
  frames = [],
  totalSamples = 100,
  selectedId,
  onSelect,
  rowHeight = 20,
  style
}) {
  const [hover, setHover] = React.useState(null);
  const depth = frames.reduce((m, fr) => Math.max(m, fr.depth), 0) + 1;
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "relative",
      width: "100%",
      height: depth * rowHeight,
      background: "var(--surface-canvas)",
      ...style
    }
  }, frames.map(fr => {
    const on = fr.id === selectedId,
      hot = fr.id === hover;
    const pct = (fr.samples / totalSamples * 100).toFixed(1);
    return /*#__PURE__*/React.createElement("div", {
      key: fr.id,
      onClick: () => onSelect && onSelect(fr),
      onMouseEnter: () => setHover(fr.id),
      onMouseLeave: () => setHover(null),
      title: fr.label + " · " + fr.samples + " samples (" + pct + "%)",
      style: {
        position: "absolute",
        left: fr.x + "%",
        width: fr.w + "%",
        top: fr.depth * rowHeight,
        height: rowHeight - 1,
        boxSizing: "border-box",
        background: fr.color || "var(--cat-deps)",
        backgroundImage: fr.inlined ? "repeating-linear-gradient(135deg,rgba(255,255,255,.22) 0 3px,transparent 3px 6px)" : undefined,
        border: on ? "1px solid var(--white)" : "1px solid var(--surface-canvas)",
        opacity: hot || on ? 1 : 0.88,
        overflow: "hidden",
        cursor: onSelect ? "pointer" : "default",
        display: "flex",
        alignItems: "center",
        padding: "0 4px",
        transition: "opacity var(--dur-instant) var(--ease-standard)"
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontSize: 10,
        color: "rgba(10,13,17,.86)",
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis"
      }
    }, fr.label));
  }));
}
Object.assign(__ds_scope, { Flamegraph });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/charts/Flamegraph.jsx", error: String((e && e.message) || e) }); }

// components/charts/ParetoScatter.jsx
try { (() => {
/** Profile Lab scatter: size vs runtime, build time encoded as point size,
    Pareto frontier drawn as a line, failed configurations greyed. */
function ParetoScatter({
  points = [],
  xLabel = "Runtime (ms)",
  yLabel = "Binary size (KB)",
  selectedId,
  onSelect,
  height = 300,
  style
}) {
  const pad = {
    l: 52,
    r: 12,
    t: 34,
    b: 30
  };
  const w = 640,
    h = height;
  const ok = points.filter(p => !p.failed);
  const xs = points.map(p => p.x),
    ys = points.map(p => p.y);
  // Pad the domains so no point can paint over the axis labels or the plot edges.
  const xSpan = Math.max(...xs) - Math.min(...xs) || 1;
  const ySpan = Math.max(...ys) - Math.min(...ys) || 1;
  const xMin = Math.min(...xs) - xSpan * 0.08,
    xMax = Math.max(...xs) + xSpan * 0.08;
  const yMin = Math.min(...ys) - ySpan * 0.12,
    yMax = Math.max(...ys) + ySpan * 0.16;
  const sx = v => pad.l + (v - xMin) / (xMax - xMin || 1) * (w - pad.l - pad.r);
  const sy = v => h - pad.b - (v - yMin) / (yMax - yMin || 1) * (h - pad.t - pad.b);
  const frontier = ok.filter(p => p.frontier).sort((a, b) => a.x - b.x);
  return /*#__PURE__*/React.createElement("svg", {
    viewBox: "0 0 " + w + " " + h,
    preserveAspectRatio: "none",
    style: {
      width: "100%",
      height,
      display: "block",
      background: "var(--surface-canvas)",
      ...style
    }
  }, [0, 0.25, 0.5, 0.75, 1].map(t => /*#__PURE__*/React.createElement("g", {
    key: t
  }, /*#__PURE__*/React.createElement("line", {
    x1: pad.l,
    x2: w - pad.r,
    y1: pad.t + t * (h - pad.t - pad.b),
    y2: pad.t + t * (h - pad.t - pad.b),
    stroke: "var(--border-subtle)",
    strokeWidth: "1"
  }))), /*#__PURE__*/React.createElement("line", {
    x1: pad.l,
    x2: pad.l,
    y1: pad.t,
    y2: h - pad.b,
    stroke: "var(--border-default)"
  }), /*#__PURE__*/React.createElement("line", {
    x1: pad.l,
    x2: w - pad.r,
    y1: h - pad.b,
    y2: h - pad.b,
    stroke: "var(--border-default)"
  }), /*#__PURE__*/React.createElement("text", {
    x: pad.l,
    y: h - 8,
    fill: "var(--text-muted)",
    style: {
      font: "11px var(--font-mono)"
    }
  }, xLabel), /*#__PURE__*/React.createElement("text", {
    x: 6,
    y: 14,
    fill: "var(--text-muted)",
    style: {
      font: "11px var(--font-mono)"
    }
  }, yLabel), frontier.length > 1 ? /*#__PURE__*/React.createElement("polyline", {
    points: frontier.map(p => sx(p.x) + "," + sy(p.y)).join(" "),
    fill: "none",
    stroke: "var(--accent)",
    strokeWidth: "1.5",
    strokeDasharray: "4 3",
    opacity: "0.8"
  }) : null, points.map(p => {
    const on = p.id === selectedId;
    const r = 4 + (p.size || 0.4) * 6;
    return /*#__PURE__*/React.createElement("g", {
      key: p.id,
      onClick: () => onSelect && onSelect(p),
      style: {
        cursor: onSelect ? "pointer" : "default"
      }
    }, /*#__PURE__*/React.createElement("circle", {
      cx: sx(p.x),
      cy: sy(p.y),
      r: r,
      fill: p.failed ? "var(--slate-700)" : p.frontier ? "var(--accent)" : "var(--status-info)",
      fillOpacity: p.failed ? 0.5 : 0.75,
      stroke: on ? "var(--white)" : p.failed ? "var(--slate-600)" : "none",
      strokeWidth: on ? 2 : 1
    }), on ? /*#__PURE__*/React.createElement("circle", {
      cx: sx(p.x),
      cy: sy(p.y),
      r: r + 5,
      fill: "none",
      stroke: "var(--accent)",
      strokeWidth: "1"
    }) : null);
  }));
}
Object.assign(__ds_scope, { ParetoScatter });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/charts/ParetoScatter.jsx", error: String((e && e.message) || e) }); }

// components/charts/Treemap.jsx
try { (() => {
/** Squarified treemap laid out in absolutely-positioned divs.
    The shipping app renders this to canvas (DESIGN §6: >5,000 elements => canvas);
    this is the cosmetic recreation for kits and mocks. */
function squarify(items, x, y, w, h, out) {
  if (!items.length) return out;
  if (items.length === 1) {
    out.push({
      ...items[0],
      x,
      y,
      w,
      h
    });
    return out;
  }
  const total = items.reduce((s, i) => s + i.value, 0);
  let acc = 0,
    split = 1;
  const target = total / 2;
  for (let i = 0; i < items.length; i++) {
    acc += items[i].value;
    if (acc >= target) {
      split = i + 1;
      break;
    }
  }
  const a = items.slice(0, split),
    b = items.slice(split);
  const ratio = acc / total;
  if (w >= h) {
    squarify(a, x, y, w * ratio, h, out);
    squarify(b, x + w * ratio, y, w * (1 - ratio), h, out);
  } else {
    squarify(a, x, y, w, h * ratio, out);
    squarify(b, x, y + h * ratio, w, h * (1 - ratio), out);
  }
  return out;
}
function Treemap({
  data = [],
  selectedId,
  onSelect,
  showLabels = true,
  height = "100%",
  style
}) {
  const items = [...data].sort((a, b) => b.value - a.value);
  const tiles = squarify(items, 0, 0, 100, 100, []);
  const [hover, setHover] = React.useState(null);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "relative",
      width: "100%",
      height,
      minHeight: 0,
      background: "var(--surface-canvas)",
      overflow: "hidden",
      ...style
    }
  }, tiles.map(t => {
    const on = t.id === selectedId;
    const hot = t.id === hover;
    return /*#__PURE__*/React.createElement("div", {
      key: t.id,
      onClick: () => onSelect && onSelect(t),
      onMouseEnter: () => setHover(t.id),
      onMouseLeave: () => setHover(null),
      title: t.label + " · " + t.display,
      style: {
        position: "absolute",
        left: t.x + "%",
        top: t.y + "%",
        width: t.w + "%",
        height: t.h + "%",
        background: t.color || "var(--cat-static)",
        opacity: hot ? 1 : on ? 1 : 0.86,
        boxSizing: "border-box",
        border: on ? "2px solid var(--white)" : "1px solid var(--surface-canvas)",
        outline: hot && !on ? "1px solid rgba(255,255,255,.5)" : "none",
        cursor: onSelect ? "pointer" : "default",
        overflow: "hidden",
        transition: "opacity var(--dur-fast) var(--ease-standard)"
      }
    }, showLabels && t.w > 9 && t.h > 12 ? /*#__PURE__*/React.createElement("div", {
      style: {
        padding: "4px 6px",
        color: "rgba(10,13,17,.86)",
        fontFamily: "var(--font-mono)",
        fontSize: 10,
        lineHeight: 1.35
      }
    }, /*#__PURE__*/React.createElement("div", {
      style: {
        fontWeight: 500,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap"
      }
    }, t.label), t.h > 20 ? /*#__PURE__*/React.createElement("div", {
      style: {
        opacity: 0.75
      }
    }, t.display) : null) : null);
  }));
}
Object.assign(__ds_scope, { Treemap });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/charts/Treemap.jsx", error: String((e && e.message) || e) }); }

// components/core/Icon.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Lucide is loaded from CDN by the host page (see readme ICONOGRAPHY).
   Lucide replaces its own placeholder nodes with <svg>, which React must never
   own — so the glyph is built imperatively and the outer <span> is the only
   React-managed element. */
function pascal(name) {
  return String(name || "").split(/[^a-z0-9]+/i).filter(Boolean).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join("");
}
function Icon({
  name,
  size = 16,
  color = "currentColor",
  strokeWidth = 1.75,
  style,
  ...rest
}) {
  const ref = React.useRef(null);
  React.useEffect(() => {
    const host = ref.current;
    const l = typeof window !== "undefined" && window.lucide;
    if (!host) return;
    host.textContent = "";
    if (!l) return;
    const node = l.icons && l.icons[pascal(name)];
    if (!node) return;
    const svg = l.createElement(node);
    svg.setAttribute("width", size);
    svg.setAttribute("height", size);
    svg.setAttribute("stroke-width", strokeWidth);
    host.appendChild(svg);
  }, [name, size, strokeWidth]);
  return /*#__PURE__*/React.createElement("span", _extends({
    ref: ref,
    "aria-hidden": "true",
    style: {
      display: "inline-flex",
      width: size,
      height: size,
      color,
      flex: "none",
      ...style
    }
  }, rest));
}
Object.assign(__ds_scope, { Icon });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Icon.jsx", error: String((e && e.message) || e) }); }

// components/analysis/EvidenceItem.jsx
try { (() => {
function EvidenceItem({
  summary,
  kind = "measured",
  tool,
  args,
  digest,
  payload,
  expanded,
  defaultOpen = false,
  onToggle,
  style
}) {
  const [open, setOpen] = React.useState(defaultOpen);
  const isOpen = expanded != null ? expanded : open;
  const toggle = () => onToggle ? onToggle(!isOpen) : setOpen(!isOpen);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      borderBottom: "1px solid var(--border-subtle)",
      ...style
    }
  }, /*#__PURE__*/React.createElement("button", {
    onClick: toggle,
    style: {
      display: "flex",
      alignItems: "flex-start",
      gap: "var(--space-8)",
      width: "100%",
      background: "none",
      border: "none",
      textAlign: "left",
      cursor: "pointer",
      padding: "var(--space-8) var(--space-4)",
      color: "inherit"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.ProvenanceBadge, {
    kind: kind,
    variant: "marker",
    style: {
      marginTop: 4
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      minWidth: 0,
      flex: 1
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      display: "block",
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-12)",
      color: "var(--text-body)"
    }
  }, summary), tool ? /*#__PURE__*/React.createElement("span", {
    style: {
      display: "block",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      marginTop: 3
    }
  }, tool, args ? " " + args : "") : null), /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: isOpen ? "chevron-down" : "chevron-right",
    size: 13,
    color: "var(--text-muted)",
    style: {
      marginTop: 3
    }
  })), isOpen ? /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "0 var(--space-4) var(--space-10) 22px"
    }
  }, /*#__PURE__*/React.createElement("pre", {
    style: {
      margin: 0,
      padding: "var(--space-8) var(--space-10)",
      background: "var(--surface-code)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-5)",
      font: "var(--type-code)",
      color: "var(--syn-plain)",
      overflow: "auto",
      maxHeight: 220,
      whiteSpace: "pre"
    }
  }, payload), digest ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-disabled)",
      marginTop: 6
    }
  }, "output digest ", digest) : null) : null);
}
Object.assign(__ds_scope, { EvidenceItem });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/EvidenceItem.jsx", error: String((e && e.message) || e) }); }

// components/analysis/FindingCard.jsx
try { (() => {
const KIND_ICON = {
  SizeDriver: "hard-drive",
  MonomorphizationBloat: "copy",
  ConfigOpportunity: "sliders-horizontal",
  CrashCause: "bug",
  PerfHotspot: "flame",
  Regression: "trending-up",
  DeadCode: "trash-2",
  StaleArtifact: "clock-alert"
};
function FindingCard({
  kind = "SizeDriver",
  title,
  detail,
  location,
  impact,
  confidence = "certain",
  provenance = "measured",
  evidenceCount,
  selected = false,
  onClick,
  footer,
  style
}) {
  const [hover, setHover] = React.useState(false);
  return /*#__PURE__*/React.createElement("article", {
    onClick: onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: {
      padding: "var(--space-10) var(--space-12)",
      borderRadius: "var(--radius-8)",
      background: selected ? "var(--surface-selected)" : hover ? "var(--surface-hover)" : "var(--surface-raised)",
      border: "1px solid " + (selected ? "var(--border-accent)" : "var(--border-subtle)"),
      cursor: onClick ? "pointer" : "default",
      transition: "var(--transition-surface)",
      minWidth: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-6)",
      marginBottom: 6
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: KIND_ICON[kind] || "circle-dot",
    size: 13,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, kind.replace(/([a-z])([A-Z])/g, "$1 $2")), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      gap: "var(--space-6)",
      alignItems: "center"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.ProvenanceBadge, {
    kind: provenance,
    variant: "marker"
  }), /*#__PURE__*/React.createElement(__ds_scope.ConfidenceBadge, {
    level: confidence
  }))), /*#__PURE__*/React.createElement("h4", {
    style: {
      margin: 0,
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-13)",
      color: "var(--text-primary)"
    }
  }, title), detail ? /*#__PURE__*/React.createElement("p", {
    style: {
      margin: "5px 0 0",
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-12)",
      color: "var(--text-secondary)"
    }
  }, detail) : null, location || impact || evidenceCount != null ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-10)",
      marginTop: 8,
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      flexWrap: "wrap"
    }
  }, location ? /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: 4
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "crosshair",
    size: 11
  }), location) : null, impact ? /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--text-accent)"
    }
  }, impact) : null, evidenceCount != null ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "inline-flex",
      alignItems: "center",
      gap: 4
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "link",
    size: 11
  }), evidenceCount, " evidence") : null) : null, footer ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 10
    }
  }, footer) : null);
}
Object.assign(__ds_scope, { FindingCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/FindingCard.jsx", error: String((e && e.message) || e) }); }

// components/analysis/GateResult.jsx
try { (() => {
const S = {
  pass: {
    icon: "check",
    color: "var(--status-pass)",
    bg: "var(--status-pass-bg)"
  },
  fail: {
    icon: "x",
    color: "var(--status-fail)",
    bg: "var(--status-fail-bg)"
  },
  skipped: {
    icon: "minus",
    color: "var(--text-muted)",
    bg: "transparent"
  },
  running: {
    icon: "loader-circle",
    color: "var(--status-running)",
    bg: "var(--accent-quiet)"
  },
  inconclusive: {
    icon: "help-circle",
    color: "var(--status-warn)",
    bg: "var(--status-warn-bg)"
  }
};
function GateResult({
  gate,
  status = "pass",
  measurement,
  note,
  style
}) {
  const s = S[status] || S.skipped;
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      height: "var(--row-h-lg)",
      padding: "0 var(--space-10)",
      borderBottom: "1px solid var(--border-subtle)",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 18,
      height: 18,
      borderRadius: "var(--radius-full)",
      display: "grid",
      placeItems: "center",
      background: s.bg,
      color: s.color,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: s.icon,
    size: 11,
    strokeWidth: 2.5
  })), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-12)",
      color: "var(--text-body)"
    }
  }, gate), note ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, note) : null, measurement ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-12)",
      color: s.color,
      fontVariantNumeric: "tabular-nums"
    }
  }, measurement) : null);
}
Object.assign(__ds_scope, { GateResult });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/GateResult.jsx", error: String((e && e.message) || e) }); }

// components/analysis/StackFrameList.jsx
try { (() => {
function StackFrameList({
  frames = [],
  selectedIndex = 0,
  onSelect,
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      minWidth: 0,
      ...style
    }
  }, frames.map((fr, i) => {
    const on = i === selectedIndex;
    return /*#__PURE__*/React.createElement("button", {
      key: i,
      onClick: () => onSelect && onSelect(fr, i),
      style: {
        display: "flex",
        alignItems: "flex-start",
        gap: "var(--space-6)",
        width: "100%",
        textAlign: "left",
        padding: "6px 10px 6px " + (10 + (fr.inlineDepth || 0) * 14) + "px",
        background: on ? "var(--surface-selected)" : "transparent",
        border: "none",
        borderBottom: "1px solid var(--border-subtle)",
        boxShadow: on ? "inset 2px 0 0 var(--accent)" : "none",
        cursor: "pointer"
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-disabled)",
        width: 20,
        flex: "none"
      }
    }, "#", i), /*#__PURE__*/React.createElement("span", {
      style: {
        minWidth: 0,
        flex: 1
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: 6
      }
    }, fr.inlineDepth ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: "corner-down-right",
      size: 11,
      color: "var(--text-muted)"
    }) : null, /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-12)",
        color: on ? "var(--text-primary)" : "var(--text-body)",
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap"
      }
    }, fr.symbol), fr.inlineDepth ? /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-caps)",
        letterSpacing: "var(--tracking-caps)",
        textTransform: "uppercase",
        color: "var(--status-info)",
        flex: "none"
      }
    }, "inlined") : null), /*#__PURE__*/React.createElement("span", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: 8,
        marginTop: 2
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, fr.location), fr.addr ? /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--syn-addr)"
      }
    }, fr.addr) : null, /*#__PURE__*/React.createElement(__ds_scope.ProvenanceBadge, {
      kind: fr.provenance || "measured",
      variant: "marker",
      detail: fr.mappingMethod,
      style: {
        marginLeft: "auto"
      }
    }))));
  }));
}
Object.assign(__ds_scope, { StackFrameList });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/StackFrameList.jsx", error: String((e && e.message) || e) }); }

// components/analysis/TranscriptStep.jsx
try { (() => {
function TranscriptStep({
  step,
  hypothesis,
  refutedBy,
  tool,
  args,
  result,
  cost,
  status = "done",
  defaultOpen = false,
  style
}) {
  const [open, setOpen] = React.useState(defaultOpen);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-8)",
      background: "var(--surface-raised)",
      overflow: "hidden",
      ...style
    }
  }, /*#__PURE__*/React.createElement("button", {
    onClick: () => setOpen(!open),
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      width: "100%",
      padding: "var(--space-8) var(--space-10)",
      background: "none",
      border: "none",
      cursor: "pointer",
      textAlign: "left"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      flex: "none"
    }
  }, "step ", step), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-12)",
      color: "var(--text-primary)",
      minWidth: 0,
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap"
    }
  }, tool), status === "running" ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "loader-circle",
    size: 12,
    color: "var(--status-running)"
  }) : null, /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)"
    }
  }, cost ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-disabled)"
    }
  }, cost) : null, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: open ? "chevron-down" : "chevron-right",
    size: 13,
    color: "var(--text-muted)"
  }))), open ? /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "0 var(--space-10) var(--space-10)",
      display: "grid",
      gap: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "var(--space-8)",
      background: "var(--prov-inferred-bg)",
      border: "1px solid rgba(255,111,21,.22)",
      borderRadius: "var(--radius-5)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-6)",
      alignItems: "center",
      marginBottom: 4
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.ProvenanceBadge, {
    kind: "inferred",
    label: "Hypothesis"
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-12)",
      color: "var(--text-body)"
    }
  }, hypothesis), refutedBy ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      marginTop: 5
    }
  }, "Would be refuted by: ", refutedBy) : null), args ? /*#__PURE__*/React.createElement("pre", {
    style: {
      margin: 0,
      padding: "var(--space-8)",
      background: "var(--surface-code)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-5)",
      font: "var(--type-code)",
      color: "var(--syn-plain)",
      overflow: "auto"
    }
  }, args) : null, result ? /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      marginBottom: 4
    }
  }, "Result"), /*#__PURE__*/React.createElement("pre", {
    style: {
      margin: 0,
      padding: "var(--space-8)",
      background: "var(--surface-code)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-5)",
      font: "var(--type-code)",
      color: "var(--syn-plain)",
      overflow: "auto",
      maxHeight: 200
    }
  }, result)) : null) : null);
}
Object.assign(__ds_scope, { TranscriptStep });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/TranscriptStep.jsx", error: String((e && e.message) || e) }); }

// components/analysis/TrustTierControl.jsx
try { (() => {
const TIERS = [{
  id: "observe",
  n: 0,
  name: "Observe",
  color: "var(--tier-observe)",
  desc: "Measures and explains. Read-only."
}, {
  id: "propose",
  n: 1,
  name: "Propose",
  color: "var(--tier-propose)",
  desc: "Generates diffs. Never applies them."
}, {
  id: "tune",
  n: 2,
  name: "Tune",
  color: "var(--tier-tune)",
  desc: "Modifies build configuration after tests pass."
}, {
  id: "autonomous",
  n: 3,
  name: "Autonomous",
  color: "var(--tier-autonomous)",
  desc: "Applies source patches, opens a PR."
}];
function TrustTierControl({
  value = "propose",
  onRequestChange,
  variant = "chip",
  open = false,
  onToggle,
  style
}) {
  const cur = TIERS.find(t => t.id === value) || TIERS[1];
  if (variant === "list") {
    return /*#__PURE__*/React.createElement("div", {
      style: {
        display: "grid",
        gap: "var(--space-4)",
        ...style
      }
    }, TIERS.map(t => {
      const on = t.id === value;
      return /*#__PURE__*/React.createElement("button", {
        key: t.id,
        onClick: () => onRequestChange && onRequestChange(t.id),
        style: {
          display: "flex",
          alignItems: "flex-start",
          gap: "var(--space-8)",
          textAlign: "left",
          padding: "var(--space-8) var(--space-10)",
          cursor: "pointer",
          background: on ? "var(--surface-selected)" : "transparent",
          border: "1px solid " + (on ? "var(--border-strong)" : "var(--border-subtle)"),
          borderLeft: "2px solid " + t.color,
          borderRadius: "var(--radius-3)"
        }
      }, /*#__PURE__*/React.createElement("span", {
        style: {
          font: "var(--type-code)",
          color: t.color,
          flex: "none"
        }
      }, t.n), /*#__PURE__*/React.createElement("span", null, /*#__PURE__*/React.createElement("span", {
        style: {
          display: "block",
          fontFamily: "var(--font-ui)",
          fontWeight: "var(--fw-semibold)",
          lineHeight: "var(--lh-snug)",
          fontSize: "var(--fs-12)",
          color: "var(--text-primary)"
        }
      }, t.name, t.id === "propose" ? " (default)" : ""), /*#__PURE__*/React.createElement("span", {
        style: {
          display: "block",
          fontFamily: "var(--font-ui)",
          fontWeight: "var(--fw-regular)",
          lineHeight: "var(--lh-normal)",
          fontSize: "var(--fs-11)",
          color: "var(--text-muted)",
          marginTop: 2
        }
      }, t.desc)), on ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
        name: "check",
        size: 13,
        color: "var(--text-accent)",
        style: {
          marginLeft: "auto"
        }
      }) : null);
    }));
  }
  return /*#__PURE__*/React.createElement("button", {
    onClick: onToggle,
    title: cur.desc,
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-6)",
      height: "var(--control-h-sm)",
      padding: "0 8px",
      background: "var(--surface-input)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-5)",
      cursor: "pointer",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 6,
      height: 6,
      borderRadius: "var(--radius-full)",
      background: cur.color
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-label)",
      color: "var(--text-body)"
    }
  }, "Trust: ", cur.name), /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "chevron-down",
    size: 12,
    color: "var(--text-muted)"
  }));
}
Object.assign(__ds_scope, { TrustTierControl });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/TrustTierControl.jsx", error: String((e && e.message) || e) }); }

// components/chrome/CommandPalette.jsx
try { (() => {
function CommandPalette({
  open = true,
  query = "",
  onQueryChange,
  groups = [],
  activeIndex = 0,
  onRun,
  onClose,
  style
}) {
  if (!open) return null;
  let flat = -1;
  return /*#__PURE__*/React.createElement("div", {
    onClick: onClose,
    style: {
      position: "absolute",
      inset: 0,
      background: "var(--surface-scrim)",
      backdropFilter: "blur(2px)",
      display: "grid",
      justifyItems: "center",
      alignItems: "start",
      paddingTop: 72,
      zIndex: 60,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    onClick: e => e.stopPropagation(),
    style: {
      width: 520,
      maxWidth: "90%",
      background: "var(--surface-overlay)",
      borderRadius: "var(--radius-12)",
      border: "1px solid var(--border-default)",
      boxShadow: "var(--shadow-overlay)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      padding: "0 var(--space-12)",
      height: 42,
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "search",
    size: 14,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("input", {
    autoFocus: true,
    value: query,
    onChange: e => onQueryChange && onQueryChange(e.target.value),
    placeholder: "Run a command, jump to a symbol\u2026",
    style: {
      flex: 1,
      background: "none",
      border: "none",
      outline: "none",
      color: "var(--text-primary)",
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-13)"
    }
  }), /*#__PURE__*/React.createElement("kbd", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-disabled)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-3)",
      padding: "1px 5px"
    }
  }, "esc")), /*#__PURE__*/React.createElement("div", {
    style: {
      maxHeight: 300,
      overflow: "auto",
      padding: "var(--space-6) 0"
    }
  }, groups.map(g => /*#__PURE__*/React.createElement("div", {
    key: g.label
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      padding: "6px var(--space-12) 4px"
    }
  }, g.label), g.items.map(it => {
    flat += 1;
    const on = flat === activeIndex;
    return /*#__PURE__*/React.createElement("button", {
      key: it.id,
      onClick: () => onRun && onRun(it),
      style: {
        display: "flex",
        alignItems: "center",
        gap: "var(--space-8)",
        width: "100%",
        textAlign: "left",
        padding: "0 var(--space-12)",
        height: 30,
        border: "none",
        cursor: "pointer",
        background: on ? "var(--surface-selected)" : "transparent",
        boxShadow: on ? "inset 2px 0 0 var(--accent)" : "none",
        color: on ? "var(--text-primary)" : "var(--text-body)"
      }
    }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: it.icon || "chevron-right",
      size: 13,
      color: on ? "var(--text-accent)" : "var(--text-muted)"
    }), /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-ui)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-normal)",
        fontSize: "var(--fs-12)"
      }
    }, it.label), it.detail ? /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, it.detail) : null, it.shortcut ? /*#__PURE__*/React.createElement("kbd", {
      style: {
        marginLeft: "auto",
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-disabled)"
      }
    }, it.shortcut) : null);
  }))))));
}
Object.assign(__ds_scope, { CommandPalette });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/chrome/CommandPalette.jsx", error: String((e && e.message) || e) }); }

// components/chrome/EmptyState.jsx
try { (() => {
function EmptyState({
  icon = "compass",
  title,
  teach,
  actions,
  footnote,
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      justifyItems: "center",
      alignContent: "center",
      gap: "var(--space-12)",
      height: "100%",
      padding: "var(--space-32)",
      textAlign: "center",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 40,
      height: 40,
      borderRadius: "var(--radius-8)",
      display: "grid",
      placeItems: "center",
      background: "var(--accent-quiet)",
      border: "1px solid rgba(255,111,21,.24)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 20,
    color: "var(--text-accent)"
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      maxWidth: 460
    }
  }, /*#__PURE__*/React.createElement("h3", {
    style: {
      margin: 0,
      fontFamily: "var(--font-display)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-18)",
      color: "var(--text-primary)",
      fontFamily: "var(--font-display)"
    }
  }, title), teach ? /*#__PURE__*/React.createElement("p", {
    style: {
      margin: "8px 0 0",
      font: "var(--type-body)",
      color: "var(--text-secondary)",
      textWrap: "pretty"
    }
  }, teach) : null), actions ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-8)"
    }
  }, actions) : null, footnote ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-disabled)"
    }
  }, footnote) : null);
}
Object.assign(__ds_scope, { EmptyState });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/chrome/EmptyState.jsx", error: String((e && e.message) || e) }); }

// components/chrome/NavRail.jsx
try { (() => {
function NavRail({
  items = [],
  value,
  onChange,
  footer,
  style
}) {
  return /*#__PURE__*/React.createElement("nav", {
    style: {
      display: "flex",
      flexDirection: "column",
      alignItems: "stretch",
      flex: "none",
      width: "var(--chrome-navrail-w)",
      padding: "var(--space-8) 0",
      background: "var(--surface-panel)",
      borderRight: "1px solid var(--border-subtle)",
      ...style
    }
  }, items.map(it => {
    const on = it.value === value;
    return /*#__PURE__*/React.createElement("button", {
      key: it.value,
      onClick: () => onChange && onChange(it.value),
      title: it.hint || it.label,
      style: {
        display: "grid",
        justifyItems: "center",
        gap: 4,
        padding: "8px 4px",
        border: "none",
        background: on ? "var(--accent-quiet)" : "transparent",
        cursor: "pointer",
        boxShadow: on ? "inset 2px 0 0 var(--accent)" : "none",
        color: on ? "var(--text-accent)" : "var(--text-muted)",
        transition: "var(--transition-control)"
      }
    }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: it.icon,
      size: 18,
      strokeWidth: on ? 2 : 1.75
    }), /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-caps)",
        letterSpacing: "var(--tracking-caps)",
        textTransform: "uppercase"
      }
    }, it.label), it.badge != null ? /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: on ? "var(--text-accent)" : "var(--text-disabled)"
      }
    }, it.badge) : null);
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: "auto",
      display: "grid",
      justifyItems: "center",
      gap: 6
    }
  }, footer));
}
Object.assign(__ds_scope, { NavRail });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/chrome/NavRail.jsx", error: String((e && e.message) || e) }); }

// components/chrome/StatusBar.jsx
try { (() => {
function StatusBar({
  status = "idle",
  message,
  progress,
  best,
  elapsed,
  onCancel,
  right,
  style
}) {
  const color = status === "running" ? "var(--status-running)" : status === "error" ? "var(--status-fail)" : status === "done" ? "var(--status-pass)" : "var(--text-muted)";
  return /*#__PURE__*/React.createElement("footer", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-10)",
      flex: "none",
      height: "var(--chrome-statusbar-h)",
      padding: "0 var(--space-12)",
      background: "var(--surface-raised)",
      borderTop: "1px solid var(--border-subtle)",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      ...style
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: status === "running" ? "zap" : status === "error" ? "triangle-alert" : status === "done" ? "check" : "circle-dot",
    size: 12,
    color: color
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      color: status === "idle" ? "var(--text-muted)" : "var(--text-body)"
    }
  }, message), progress ? /*#__PURE__*/React.createElement("span", null, progress) : null, best ? /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--delta-improve)"
    }
  }, "best so far: ", best) : null, elapsed ? /*#__PURE__*/React.createElement("span", null, elapsed) : null, onCancel && status === "running" ? /*#__PURE__*/React.createElement("button", {
    onClick: onCancel,
    style: {
      background: "none",
      border: "none",
      color: "var(--text-link)",
      cursor: "pointer",
      font: "inherit",
      padding: 0
    }
  }, "cancel") : null, /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-10)"
    }
  }, right));
}
Object.assign(__ds_scope, { StatusBar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/chrome/StatusBar.jsx", error: String((e && e.message) || e) }); }

// components/chrome/TitleBar.jsx
try { (() => {
function TitleBar({
  project,
  commit,
  dirty = false,
  left,
  right,
  style
}) {
  return /*#__PURE__*/React.createElement("header", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-10)",
      flex: "none",
      height: "var(--chrome-titlebar-h)",
      padding: "0 var(--space-12)",
      background: "var(--surface-raised)",
      borderBottom: "1px solid var(--border-subtle)",
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-6)",
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "crosshair",
    size: 14,
    color: "var(--text-accent)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-13)",
      color: "var(--text-primary)",
      whiteSpace: "nowrap"
    }
  }, project), commit ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "@ ", commit) : null, dirty ? /*#__PURE__*/React.createElement("span", {
    title: "Working tree is dirty \u2014 binary and source may not correspond",
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--status-warn)"
    }
  }, "dirty") : null), left, /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)"
    }
  }, right));
}
Object.assign(__ds_scope, { TitleBar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/chrome/TitleBar.jsx", error: String((e && e.message) || e) }); }

// components/core/Badge.jsx
try { (() => {
const TONES = {
  neutral: ["var(--prov-derived-bg)", "var(--text-secondary)", "var(--border-default)"],
  pass: ["var(--status-pass-bg)", "var(--status-pass)", "rgba(70,192,141,.35)"],
  fail: ["var(--status-fail-bg)", "var(--status-fail)", "rgba(229,86,91,.35)"],
  warn: ["var(--status-warn-bg)", "var(--status-warn)", "rgba(229,169,60,.35)"],
  info: ["var(--status-info-bg)", "var(--status-info)", "rgba(91,143,185,.35)"],
  accent: ["var(--accent-quiet)", "var(--text-accent)", "rgba(255,111,21,.32)"]
};
function Badge({
  children,
  tone = "neutral",
  icon,
  mono = false,
  outline = false,
  style
}) {
  const [bg, fg, bd] = TONES[tone] || TONES.neutral;
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-4)",
      padding: "1px 6px",
      background: outline ? "transparent" : bg,
      color: fg,
      border: "1px solid " + bd,
      borderRadius: "var(--radius-2)",
      font: mono ? "var(--type-code)" : "var(--type-caps)",
      fontSize: "var(--fs-11)",
      letterSpacing: mono ? 0 : "var(--tracking-caps)",
      textTransform: mono ? "none" : "uppercase",
      whiteSpace: "nowrap",
      ...style
    }
  }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 11
  }) : null, children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Badge.jsx", error: String((e && e.message) || e) }); }

// components/agents/ModelPicker.jsx
try { (() => {
/** Grouped model list. Models are ALWAYS sorted newest-first within a group by
    `released` (ISO date), so the latest model a provider ships sits on top and the
    newest overall carries the "latest" badge. Callers never pre-sort. */
function byNewest(a, b) {
  return String(b.released || "").localeCompare(String(a.released || ""));
}
function ModelPicker({
  groups = [],
  value,
  onChange,
  filter = "",
  onFilterChange,
  showPricing = true,
  height = 300,
  style
}) {
  const q = filter.trim().toLowerCase();
  const sorted = groups.map(g => ({
    ...g,
    models: [...(g.models || [])].sort(byNewest).filter(m => !q || (m.id + " " + m.name + " " + (g.provider || "")).toLowerCase().includes(q))
  })).filter(g => g.models.length);
  const newest = sorted.flatMap(g => g.models).sort(byNewest)[0];
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      flexDirection: "column",
      minHeight: 0,
      minWidth: 0,
      ...style
    }
  }, onFilterChange ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 6,
      flex: "none",
      height: 28,
      padding: "0 8px",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "search",
    size: 13,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("input", {
    value: filter,
    onChange: e => onFilterChange(e.target.value),
    placeholder: "Filter models\u2026",
    style: {
      flex: 1,
      background: "none",
      border: "none",
      outline: "none",
      color: "var(--text-primary)",
      font: "var(--type-code)"
    }
  })) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minHeight: 0,
      height,
      overflow: "auto"
    }
  }, sorted.map(g => /*#__PURE__*/React.createElement("div", {
    key: g.provider
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      position: "sticky",
      top: 0,
      zIndex: 1,
      display: "flex",
      alignItems: "center",
      gap: 6,
      padding: "6px 10px",
      background: "var(--surface-raised)",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, g.provider), g.transport ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-disabled)"
    }
  }, g.transport) : null, g.connected === false ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: "warn"
  }, "no key")) : null), g.models.map(m => {
    const on = m.id === value;
    const disabled = g.connected === false || m.available === false;
    return /*#__PURE__*/React.createElement("button", {
      key: m.id,
      onClick: () => !disabled && onChange && onChange(m.id, g),
      disabled: disabled,
      style: {
        display: "grid",
        gridTemplateColumns: "1fr auto",
        gap: 4,
        width: "100%",
        textAlign: "left",
        padding: "7px 10px",
        border: "none",
        cursor: disabled ? "not-allowed" : "pointer",
        background: on ? "var(--surface-selected)" : "transparent",
        boxShadow: on ? "inset 2px 0 0 var(--accent)" : "none",
        borderBottom: "1px solid var(--border-subtle)",
        opacity: disabled ? 0.45 : 1,
        transition: "background-color var(--dur-instant) var(--ease-standard)"
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        minWidth: 0
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: 6
      }
    }, /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-ui)",
        fontWeight: "var(--fw-semibold)",
        lineHeight: "var(--lh-snug)",
        fontSize: "var(--fs-12)",
        color: on ? "var(--text-primary)" : "var(--text-body)"
      }
    }, m.name), newest && m.id === newest.id ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
      tone: "accent"
    }, "latest") : null, m.tags ? m.tags.map(t => /*#__PURE__*/React.createElement(__ds_scope.Badge, {
      key: t,
      tone: "neutral"
    }, t)) : null, m.available === false ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
      tone: "warn"
    }, "invite only") : null), /*#__PURE__*/React.createElement("span", {
      style: {
        display: "block",
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)",
        marginTop: 2,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap"
      }
    }, m.id, m.context ? " \u00b7 " + m.context : "", m.released ? " \u00b7 " + m.released : "")), /*#__PURE__*/React.createElement("span", {
      style: {
        textAlign: "right",
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)",
        whiteSpace: "nowrap"
      }
    }, showPricing && m.price ? m.price : null, on ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: "check",
      size: 13,
      color: "var(--text-accent)",
      style: {
        marginLeft: 6,
        verticalAlign: "-2px"
      }
    }) : null));
  }))), !sorted.length ? /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "var(--space-16)",
      textAlign: "center",
      font: "var(--type-body)",
      color: "var(--text-muted)"
    }
  }, "No models match.") : null));
}
Object.assign(__ds_scope, { ModelPicker });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agents/ModelPicker.jsx", error: String((e && e.message) || e) }); }

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const H = {
  sm: "var(--control-h-sm)",
  md: "var(--control-h-md)",
  lg: "var(--control-h-lg)"
};
const PAD = {
  sm: "0 8px",
  md: "0 12px",
  lg: "0 16px"
};
const FS = {
  sm: "var(--fs-11)",
  md: "var(--fs-12)",
  lg: "var(--fs-13)"
};
const VARIANTS = {
  primary: {
    background: "var(--accent)",
    color: "var(--on-accent)",
    border: "1px solid var(--accent)"
  },
  secondary: {
    background: "var(--surface-raised)",
    color: "var(--text-primary)",
    border: "1px solid var(--border-default)"
  },
  ghost: {
    background: "transparent",
    color: "var(--text-secondary)",
    border: "1px solid transparent"
  },
  danger: {
    background: "transparent",
    color: "var(--status-fail)",
    border: "1px solid rgba(229,86,91,.45)"
  },
  accentQuiet: {
    background: "var(--accent-quiet)",
    color: "var(--text-accent)",
    border: "1px solid rgba(255,111,21,.28)"
  }
};
const HOVER = {
  primary: {
    background: "var(--accent-hover)",
    borderColor: "var(--accent-hover)"
  },
  secondary: {
    background: "var(--surface-hover)",
    borderColor: "var(--border-strong)"
  },
  ghost: {
    background: "var(--surface-hover)",
    color: "var(--text-primary)"
  },
  danger: {
    background: "var(--status-fail-bg)"
  },
  accentQuiet: {
    background: "var(--accent-quiet-hover)"
  }
};
function Button({
  children,
  variant = "secondary",
  size = "md",
  icon,
  iconRight,
  disabled = false,
  loading = false,
  fullWidth = false,
  onClick,
  type = "button",
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const [press, setPress] = React.useState(false);
  const v = VARIANTS[variant] || VARIANTS.secondary;
  return /*#__PURE__*/React.createElement("button", _extends({
    type: type,
    disabled: disabled || loading,
    onClick: onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => {
      setHover(false);
      setPress(false);
    },
    onMouseDown: () => setPress(true),
    onMouseUp: () => setPress(false),
    style: {
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      gap: "var(--space-6)",
      height: H[size],
      padding: PAD[size],
      width: fullWidth ? "100%" : undefined,
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-medium)",
      lineHeight: "var(--lh-snug)",
      fontSize: FS[size],
      borderRadius: "var(--radius-5)",
      cursor: disabled || loading ? "not-allowed" : "pointer",
      whiteSpace: "nowrap",
      transition: "var(--transition-control)",
      opacity: disabled ? 0.45 : 1,
      transform: press && !disabled ? "translateY(1px)" : "none",
      ...v,
      ...(hover && !disabled && !loading ? HOVER[variant] : null),
      ...style
    }
  }, rest), loading ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "loader-circle",
    size: size === "sm" ? 12 : 14
  }) : icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: size === "sm" ? 12 : 14
  }) : null, children, iconRight ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: iconRight,
    size: size === "sm" ? 12 : 14
  }) : null);
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/agents/AcpAgentCard.jsx
try { (() => {
/** An external coding agent connected over ACP (JSON-RPC 2.0 on stdio).
    ACP agents own their own auth and billing, so Binmap shows their state
    rather than storing their credentials. */
const STATE = {
  connected: {
    tone: "pass",
    label: "connected",
    icon: "plug-zap"
  },
  installed: {
    tone: "info",
    label: "installed",
    icon: "package-check"
  },
  needsLogin: {
    tone: "warn",
    label: "login required",
    icon: "lock-keyhole"
  },
  missing: {
    tone: "neutral",
    label: "not installed",
    icon: "download"
  },
  error: {
    tone: "fail",
    label: "handshake failed",
    icon: "triangle-alert"
  }
};
function AcpAgentCard({
  name,
  vendor,
  command,
  state = "missing",
  version,
  protocol = "ACP 1.0 · stdio",
  capabilities = [],
  authNote,
  selected = false,
  onSelect,
  onConnect,
  onLogin,
  error,
  style
}) {
  const s = STATE[state] || STATE.missing;
  return /*#__PURE__*/React.createElement("section", {
    onClick: onSelect,
    style: {
      padding: "var(--space-10) var(--space-12)",
      borderRadius: "var(--radius-8)",
      background: selected ? "var(--surface-selected)" : "var(--surface-raised)",
      border: "1px solid " + (selected ? "var(--border-accent)" : "var(--border-subtle)"),
      cursor: onSelect ? "pointer" : "default",
      minWidth: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      flexWrap: "wrap"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "terminal",
    size: 14,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-13)",
      color: "var(--text-primary)"
    }
  }, name), vendor ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, vendor) : null, /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: s.tone,
    icon: s.icon
  }, s.label), version ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    mono: true,
    tone: "neutral"
  }, version) : null), command ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 8,
      padding: "6px 8px",
      background: "var(--surface-code)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-3)",
      font: "var(--type-code)",
      color: "var(--syn-plain)",
      overflow: "auto",
      whiteSpace: "pre"
    }
  }, command) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      marginTop: 8,
      flexWrap: "wrap",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, /*#__PURE__*/React.createElement("span", null, protocol), capabilities.map(c => /*#__PURE__*/React.createElement("span", {
    key: c,
    style: {
      color: "var(--text-secondary)"
    }
  }, c))), authNote ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 8,
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, authNote) : null, error ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 8,
      padding: "6px 8px",
      background: "var(--status-fail-bg)",
      border: "1px solid rgba(229,86,91,.35)",
      borderRadius: "var(--radius-3)",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--status-fail)",
      whiteSpace: "pre-wrap"
    }
  }, error) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-8)",
      marginTop: 10
    }
  }, state === "missing" ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "secondary",
    icon: "download"
  }, "Install adapter") : null, state === "needsLogin" ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "primary",
    icon: "log-in",
    onClick: onLogin
  }, "Run /login") : null, state === "installed" || state === "error" ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "primary",
    icon: "plug-zap",
    onClick: onConnect
  }, "Connect") : null, state === "connected" ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "secondary",
    icon: "rotate-cw",
    onClick: onConnect
  }, "Re-handshake") : null, /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "ghost",
    icon: "file-code"
  }, "Edit agent config")));
}
Object.assign(__ds_scope, { AcpAgentCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agents/AcpAgentCard.jsx", error: String((e && e.message) || e) }); }

// components/analysis/ProposalBlock.jsx
try { (() => {
function ProposalBlock({
  summary,
  diff = [],
  tier = "Propose",
  blocked,
  onVerify,
  onApply,
  verifying = false,
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-8)",
      background: "var(--surface-panel)",
      overflow: "hidden",
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      padding: "var(--space-8) var(--space-10)",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "git-pull-request-arrow",
    size: 13,
    color: "var(--text-accent)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-12)",
      color: "var(--text-primary)"
    }
  }, summary), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "tier: ", tier)), /*#__PURE__*/React.createElement("pre", {
    style: {
      margin: 0,
      font: "var(--type-code)",
      background: "var(--surface-code)",
      overflow: "auto",
      maxHeight: 180
    }
  }, diff.map((l, i) => {
    const add = l.startsWith("+"),
      del = l.startsWith("-"),
      hunk = l.startsWith("@@");
    return /*#__PURE__*/React.createElement("div", {
      key: i,
      style: {
        padding: "0 10px",
        background: add ? "rgba(70,192,141,.08)" : del ? "rgba(229,86,91,.08)" : "transparent",
        color: add ? "var(--status-pass)" : del ? "var(--status-fail)" : hunk ? "var(--text-muted)" : "var(--syn-plain)"
      }
    }, l);
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      padding: "var(--space-8) var(--space-10)",
      borderTop: "1px solid var(--border-subtle)",
      background: "var(--surface-raised)"
    }
  }, blocked ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--status-warn)",
      display: "inline-flex",
      alignItems: "center",
      gap: 5
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "lock",
    size: 12
  }), blocked) : null, /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      gap: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "secondary",
    icon: "clipboard-copy"
  }, "Copy patch"), onVerify ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "primary",
    icon: "check-check",
    loading: verifying,
    onClick: onVerify
  }, verifying ? "Verifying…" : "Verify") : null, onApply ? /*#__PURE__*/React.createElement(__ds_scope.Button, {
    size: "sm",
    variant: "secondary",
    icon: "git-commit-horizontal",
    onClick: onApply,
    disabled: !!blocked
  }, "Apply") : null)));
}
Object.assign(__ds_scope, { ProposalBlock });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/analysis/ProposalBlock.jsx", error: String((e && e.message) || e) }); }

// components/core/Checkbox.jsx
try { (() => {
function Checkbox({
  checked = false,
  onChange,
  label,
  description,
  disabled = false,
  style
}) {
  return /*#__PURE__*/React.createElement("label", {
    style: {
      display: "inline-flex",
      alignItems: description ? "flex-start" : "center",
      gap: "var(--space-8)",
      cursor: disabled ? "not-allowed" : "pointer",
      opacity: disabled ? 0.5 : 1,
      ...style
    }
  }, /*#__PURE__*/React.createElement("input", {
    type: "checkbox",
    checked: checked,
    disabled: disabled,
    onChange: e => onChange && onChange(e.target.checked, e),
    style: {
      position: "absolute",
      opacity: 0,
      width: 0,
      height: 0
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      width: 14,
      height: 14,
      flex: "none",
      marginTop: description ? 2 : 0,
      display: "grid",
      placeItems: "center",
      borderRadius: "var(--radius-2)",
      background: checked ? "var(--accent)" : "var(--surface-input)",
      border: "1px solid " + (checked ? "var(--accent)" : "var(--border-strong)"),
      transition: "var(--transition-control)"
    }
  }, checked ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "check",
    size: 10,
    color: "var(--on-accent)",
    strokeWidth: 3
  }) : null), label ? /*#__PURE__*/React.createElement("span", null, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-label)",
      color: "var(--text-body)"
    }
  }, label), description ? /*#__PURE__*/React.createElement("span", {
    style: {
      display: "block",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      marginTop: 2
    }
  }, description) : null) : null);
}
Object.assign(__ds_scope, { Checkbox });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Checkbox.jsx", error: String((e && e.message) || e) }); }

// components/core/IconButton.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const S = {
  sm: 24,
  md: 30,
  lg: 36
};
function IconButton({
  icon,
  label,
  size = "md",
  active = false,
  disabled = false,
  onClick,
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  return /*#__PURE__*/React.createElement("button", _extends({
    type: "button",
    title: label,
    "aria-label": label,
    "aria-pressed": active,
    disabled: disabled,
    onClick: onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: {
      width: S[size],
      height: S[size],
      display: "inline-grid",
      placeItems: "center",
      borderRadius: "var(--radius-5)",
      cursor: disabled ? "not-allowed" : "pointer",
      border: "1px solid " + (active ? "rgba(255,111,21,.28)" : "transparent"),
      background: active ? "var(--accent-quiet)" : hover && !disabled ? "var(--surface-hover)" : "transparent",
      color: active ? "var(--text-accent)" : hover && !disabled ? "var(--text-primary)" : "var(--text-secondary)",
      opacity: disabled ? 0.4 : 1,
      transition: "var(--transition-control)",
      ...style
    }
  }, rest), /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: size === "sm" ? 13 : size === "lg" ? 17 : 15
  }));
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/agents/ApiKeyField.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/** Secrets go to the OS keyring, never to a config file in the repo (DESIGN §9).
    The field therefore shows a stored key as a masked fingerprint, not as text. */
function ApiKeyField({
  value = "",
  onChange,
  placeholder = "sk-…",
  stored = false,
  fingerprint,
  envVar,
  keyring = true,
  onTest,
  status = "idle",
  onClear,
  style
}) {
  const [reveal, setReveal] = React.useState(false);
  const [focus, setFocus] = React.useState(false);
  const tone = status === "valid" ? "var(--status-pass)" : status === "invalid" ? "var(--status-fail)" : status === "testing" ? "var(--status-running)" : "var(--text-muted)";
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6,
      minWidth: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-6)",
      height: "var(--control-h-md)",
      padding: "0 6px 0 8px",
      background: "var(--surface-input)",
      borderRadius: "var(--radius-3)",
      border: "1px solid " + (status === "invalid" ? "var(--status-fail)" : focus ? "var(--focus-ring)" : "var(--border-default)"),
      boxShadow: focus ? "0 0 0 2px rgba(255,111,21,.16)" : "none",
      transition: "var(--transition-control)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: stored ? "key-round" : "key",
    size: 13,
    color: stored ? "var(--status-pass)" : "var(--text-muted)"
  }), stored ? /*#__PURE__*/React.createElement("span", {
    style: {
      flex: 1,
      minWidth: 0,
      font: "var(--type-code)",
      color: "var(--text-body)",
      overflow: "hidden",
      textOverflow: "ellipsis"
    }
  }, fingerprint || "\u2022\u2022\u2022\u2022 stored in keyring") : /*#__PURE__*/React.createElement("input", _extends({
    type: reveal ? "text" : "password",
    placeholder: placeholder
  }, onChange ? {
    value,
    onChange
  } : {
    defaultValue: value
  }, {
    onFocus: () => setFocus(true),
    onBlur: () => setFocus(false),
    spellCheck: "false",
    autoComplete: "off",
    style: {
      flex: 1,
      minWidth: 0,
      background: "none",
      border: "none",
      outline: "none",
      padding: 0,
      color: "var(--text-primary)",
      font: "var(--type-code)"
    }
  })), !stored ? /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    size: "sm",
    icon: reveal ? "eye-off" : "eye",
    label: reveal ? "Hide key" : "Reveal key",
    onClick: () => setReveal(!reveal)
  }) : null, stored && onClear ? /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    size: "sm",
    icon: "trash-2",
    label: "Remove key from keyring",
    onClick: onClear
  }) : null, onTest ? /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    size: "sm",
    icon: status === "testing" ? "loader-circle" : "plug-zap",
    label: "Test connection",
    active: status === "valid",
    onClick: onTest
  }) : null), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      flexWrap: "wrap"
    }
  }, keyring ? /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: 4
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "lock",
    size: 11
  }), "OS keyring") : null, envVar ? /*#__PURE__*/React.createElement("span", null, "or ", envVar) : null, status !== "idle" ? /*#__PURE__*/React.createElement("span", {
    style: {
      color: tone,
      marginLeft: "auto"
    }
  }, status === "valid" ? "connection ok" : status === "invalid" ? "rejected" : "testing\u2026") : null));
}
Object.assign(__ds_scope, { ApiKeyField });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agents/ApiKeyField.jsx", error: String((e && e.message) || e) }); }

// components/core/Dialog.jsx
try { (() => {
function Dialog({
  open = true,
  title,
  description,
  children,
  footer,
  width = 460,
  onClose,
  tone = "neutral",
  style
}) {
  if (!open) return null;
  const toneColor = tone === "danger" ? "var(--status-fail)" : tone === "warn" ? "var(--status-warn)" : "var(--border-default)";
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "absolute",
      inset: 0,
      background: "var(--surface-scrim)",
      backdropFilter: "blur(2px)",
      display: "grid",
      placeItems: "center",
      zIndex: 40,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    role: "dialog",
    "aria-modal": "true",
    style: {
      width,
      maxWidth: "92%",
      background: "var(--surface-overlay)",
      borderRadius: "var(--radius-12)",
      border: "1px solid " + toneColor,
      boxShadow: "var(--shadow-overlay)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement("header", {
    style: {
      display: "flex",
      alignItems: "flex-start",
      gap: "var(--space-8)",
      padding: "var(--space-16) var(--space-16) var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement("h2", {
    style: {
      margin: 0,
      fontFamily: "var(--font-display)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-16)",
      color: "var(--text-primary)"
    }
  }, title), description ? /*#__PURE__*/React.createElement("p", {
    style: {
      margin: "6px 0 0",
      font: "var(--type-body)",
      color: "var(--text-secondary)"
    }
  }, description) : null), onClose ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.IconButton, {
    icon: "x",
    label: "Close",
    size: "sm",
    onClick: onClose
  })) : null), children ? /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "var(--space-8) var(--space-16) var(--space-16)"
    }
  }, children) : null, footer ? /*#__PURE__*/React.createElement("footer", {
    style: {
      display: "flex",
      justifyContent: "flex-end",
      gap: "var(--space-8)",
      padding: "var(--space-12) var(--space-16)",
      borderTop: "1px solid var(--border-subtle)",
      background: "var(--surface-raised)"
    }
  }, footer) : null));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/core/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function Input({
  value,
  onChange,
  placeholder,
  icon,
  mono = false,
  size = "md",
  invalid = false,
  disabled = false,
  suffix,
  fullWidth = true,
  type = "text",
  style,
  ...rest
}) {
  const [focus, setFocus] = React.useState(false);
  const h = size === "sm" ? "var(--control-h-sm)" : size === "lg" ? "var(--control-h-lg)" : "var(--control-h-md)";
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-6)",
      height: h,
      padding: "0 8px",
      width: fullWidth ? "100%" : undefined,
      boxSizing: "border-box",
      background: "var(--surface-input)",
      borderRadius: "var(--radius-3)",
      border: "1px solid " + (invalid ? "var(--status-fail)" : focus ? "var(--focus-ring)" : "var(--border-default)"),
      boxShadow: focus ? "0 0 0 2px rgba(255,111,21,.16)" : "none",
      opacity: disabled ? 0.5 : 1,
      transition: "var(--transition-control)",
      ...style
    }
  }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 13,
    color: "var(--text-muted)"
  }) : null, /*#__PURE__*/React.createElement("input", _extends({
    type: type,
    value: value,
    placeholder: placeholder,
    disabled: disabled,
    onChange: onChange,
    onFocus: () => setFocus(true),
    onBlur: () => setFocus(false),
    style: {
      flex: 1,
      minWidth: 0,
      background: "none",
      border: "none",
      outline: "none",
      padding: 0,
      color: "var(--text-primary)",
      fontSize: size === "lg" ? "var(--fs-13)" : "var(--fs-12)",
      fontFamily: mono ? "var(--font-mono)" : "var(--font-ui)"
    }
  }, rest)), suffix ? /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      color: "var(--text-muted)",
      flex: "none"
    }
  }, suffix) : null);
}
Object.assign(__ds_scope, { Input });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Input.jsx", error: String((e && e.message) || e) }); }

// components/agents/ProviderCard.jsx
try { (() => {
/** One keyed model provider: native (Anthropic, OpenAI) or any OpenAI-compatible endpoint. */
function ProviderCard({
  name,
  kind = "openai-compatible",
  baseUrl,
  docsHint,
  envVar,
  keyStored = false,
  keyFingerprint,
  apiKey,
  onApiKeyChange,
  status = "idle",
  modelCount,
  selected = false,
  onSelect,
  onTest,
  onClear,
  editableBaseUrl = false,
  onBaseUrlChange,
  cloudBlocked = false,
  footer,
  style
}) {
  const tone = status === "valid" ? "pass" : status === "invalid" ? "fail" : keyStored ? "info" : "neutral";
  const label = status === "valid" ? "connected" : status === "invalid" ? "rejected" : keyStored ? "key stored" : "not configured";
  return /*#__PURE__*/React.createElement("section", {
    onClick: onSelect,
    style: {
      padding: "var(--space-10) var(--space-12)",
      borderRadius: "var(--radius-8)",
      background: selected ? "var(--surface-selected)" : "var(--surface-raised)",
      border: "1px solid " + (selected ? "var(--border-accent)" : "var(--border-subtle)"),
      cursor: onSelect ? "pointer" : "default",
      minWidth: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: kind === "native" ? "shield-check" : kind === "local" ? "cpu" : "cloud",
    size: 14,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-13)",
      color: "var(--text-primary)"
    }
  }, name), /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: tone
  }, label), cloudBlocked ? /*#__PURE__*/React.createElement(__ds_scope.Badge, {
    tone: "warn",
    icon: "lock"
  }, "cloud disabled for this project") : null, modelCount != null ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, modelCount, " models") : null), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 8,
      marginTop: 10
    }
  }, editableBaseUrl ? /*#__PURE__*/React.createElement(__ds_scope.Input, {
    mono: true,
    size: "sm",
    value: baseUrl,
    onChange: onBaseUrlChange,
    placeholder: "https://host/v1",
    icon: "link"
  }) : baseUrl ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-mono)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, baseUrl) : null, /*#__PURE__*/React.createElement(__ds_scope.ApiKeyField, {
    stored: keyStored,
    fingerprint: keyFingerprint,
    envVar: envVar,
    status: status,
    value: apiKey,
    onChange: onApiKeyChange,
    onTest: onTest,
    onClear: onClear
  }), docsHint ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-regular)",
      lineHeight: "var(--lh-normal)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, docsHint) : null, footer));
}
Object.assign(__ds_scope, { ProviderCard });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/agents/ProviderCard.jsx", error: String((e && e.message) || e) }); }

// components/core/Panel.jsx
try { (() => {
function Panel({
  title,
  eyebrow,
  icon,
  actions,
  children,
  padding = true,
  scroll = false,
  footer,
  style,
  bodyStyle
}) {
  return /*#__PURE__*/React.createElement("section", {
    style: {
      display: "flex",
      flexDirection: "column",
      minHeight: 0,
      minWidth: 0,
      background: "var(--surface-panel)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-8)",
      overflow: "hidden",
      ...style
    }
  }, title || actions || eyebrow ? /*#__PURE__*/React.createElement("header", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-8)",
      flex: "none",
      padding: "0 var(--space-12)",
      height: 34,
      borderBottom: "1px solid var(--border-subtle)",
      background: "var(--surface-raised)"
    }
  }, icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: 14,
    color: "var(--text-muted)"
  }) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      minWidth: 0
    }
  }, eyebrow ? /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, eyebrow) : null, title ? /*#__PURE__*/React.createElement("h3", {
    style: {
      margin: 0,
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-semibold)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-12)",
      color: "var(--text-primary)",
      whiteSpace: "nowrap",
      overflow: "hidden",
      textOverflow: "ellipsis"
    }
  }, title) : null), /*#__PURE__*/React.createElement("div", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: "var(--space-4)"
    }
  }, actions)) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minHeight: 0,
      padding: padding ? "var(--space-12)" : 0,
      overflow: scroll ? "auto" : "hidden",
      ...bodyStyle
    }
  }, children), footer ? /*#__PURE__*/React.createElement("footer", {
    style: {
      flex: "none",
      padding: "var(--space-8) var(--space-12)",
      borderTop: "1px solid var(--border-subtle)",
      background: "var(--surface-raised)"
    }
  }, footer) : null);
}
Object.assign(__ds_scope, { Panel });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Panel.jsx", error: String((e && e.message) || e) }); }

// components/core/SegmentedControl.jsx
try { (() => {
function SegmentedControl({
  value,
  onChange,
  options = [],
  size = "md",
  fullWidth = false,
  style
}) {
  const h = size === "sm" ? "var(--control-h-sm)" : "var(--control-h-md)";
  return /*#__PURE__*/React.createElement("div", {
    role: "tablist",
    style: {
      display: "inline-flex",
      height: h,
      padding: 2,
      gap: 2,
      boxSizing: "border-box",
      background: "var(--surface-input)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-5)",
      width: fullWidth ? "100%" : undefined,
      ...style
    }
  }, options.map(o => {
    const opt = typeof o === "string" ? {
      value: o,
      label: o
    } : o;
    const on = opt.value === value;
    return /*#__PURE__*/React.createElement("button", {
      key: opt.value,
      role: "tab",
      "aria-selected": on,
      onClick: () => onChange && onChange(opt.value),
      style: {
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-4)",
        flex: fullWidth ? 1 : "none",
        justifyContent: "center",
        padding: "0 10px",
        border: "none",
        cursor: "pointer",
        borderRadius: "var(--radius-3)",
        fontFamily: "var(--font-ui)",
        fontWeight: "var(--fw-medium)",
        lineHeight: "var(--lh-snug)",
        fontSize: size === "sm" ? "var(--fs-11)" : "var(--fs-12)",
        background: on ? "var(--surface-active)" : "transparent",
        color: on ? "var(--text-primary)" : "var(--text-muted)",
        boxShadow: on ? "var(--shadow-raised)" : "none",
        transition: "var(--transition-control)"
      }
    }, opt.icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: opt.icon,
      size: 12
    }) : null, opt.label);
  }));
}
Object.assign(__ds_scope, { SegmentedControl });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/SegmentedControl.jsx", error: String((e && e.message) || e) }); }

// components/core/Select.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
function Select({
  value,
  onChange,
  options = [],
  size = "md",
  disabled = false,
  fullWidth = false,
  style,
  ...rest
}) {
  const [hover, setHover] = React.useState(false);
  const h = size === "sm" ? "var(--control-h-sm)" : size === "lg" ? "var(--control-h-lg)" : "var(--control-h-md)";
  return /*#__PURE__*/React.createElement("div", {
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: {
      position: "relative",
      display: "inline-flex",
      alignItems: "center",
      height: h,
      width: fullWidth ? "100%" : undefined,
      background: "var(--surface-input)",
      border: "1px solid " + (hover && !disabled ? "var(--border-strong)" : "var(--border-default)"),
      borderRadius: "var(--radius-3)",
      opacity: disabled ? 0.5 : 1,
      transition: "var(--transition-control)",
      ...style
    }
  }, /*#__PURE__*/React.createElement("select", _extends({
    value: value,
    onChange: onChange,
    disabled: disabled,
    style: {
      appearance: "none",
      background: "none",
      border: "none",
      outline: "none",
      color: "var(--text-primary)",
      fontFamily: "var(--font-ui)",
      fontWeight: "var(--fw-medium)",
      lineHeight: "var(--lh-snug)",
      fontSize: "var(--fs-12)",
      padding: "0 26px 0 10px",
      height: "100%",
      width: "100%",
      cursor: disabled ? "not-allowed" : "pointer"
    }
  }, rest), options.map(o => {
    const opt = typeof o === "string" ? {
      value: o,
      label: o
    } : o;
    return /*#__PURE__*/React.createElement("option", {
      key: opt.value,
      value: opt.value
    }, opt.label);
  })), /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "chevron-down",
    size: 13,
    color: "var(--text-muted)",
    style: {
      position: "absolute",
      right: 7,
      pointerEvents: "none"
    }
  }));
}
Object.assign(__ds_scope, { Select });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Select.jsx", error: String((e && e.message) || e) }); }

// components/core/Switch.jsx
try { (() => {
function Switch({
  checked = false,
  onChange,
  label,
  disabled = false,
  size = "md",
  style
}) {
  const w = size === "sm" ? 26 : 32,
    h = size === "sm" ? 14 : 18,
    k = h - 4;
  return /*#__PURE__*/React.createElement("label", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-8)",
      cursor: disabled ? "not-allowed" : "pointer",
      opacity: disabled ? 0.5 : 1,
      ...style
    }
  }, /*#__PURE__*/React.createElement("input", {
    type: "checkbox",
    checked: checked,
    disabled: disabled,
    onChange: e => onChange && onChange(e.target.checked, e),
    style: {
      position: "absolute",
      opacity: 0,
      width: 0,
      height: 0
    }
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      width: w,
      height: h,
      borderRadius: "var(--radius-full)",
      flex: "none",
      position: "relative",
      background: checked ? "var(--accent)" : "var(--surface-active)",
      border: "1px solid " + (checked ? "var(--accent)" : "var(--border-default)"),
      transition: "background-color var(--dur-fast) var(--ease-standard)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      position: "absolute",
      top: 1,
      left: checked ? w - k - 3 : 1,
      width: k,
      height: k,
      borderRadius: "var(--radius-full)",
      background: checked ? "var(--on-accent)" : "var(--slate-300)",
      transition: "left var(--dur-fast) var(--ease-standard)"
    }
  })), label ? /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-label)",
      color: "var(--text-body)"
    }
  }, label) : null);
}
Object.assign(__ds_scope, { Switch });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Switch.jsx", error: String((e && e.message) || e) }); }

// components/core/Tabs.jsx
try { (() => {
function Tabs({
  value,
  onChange,
  tabs = [],
  variant = "underline",
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    role: "tablist",
    style: {
      display: "flex",
      gap: variant === "underline" ? "var(--space-16)" : "var(--space-4)",
      borderBottom: variant === "underline" ? "1px solid var(--border-subtle)" : "none",
      alignItems: "stretch",
      ...style
    }
  }, tabs.map(t => {
    const tab = typeof t === "string" ? {
      value: t,
      label: t
    } : t;
    const on = tab.value === value;
    return /*#__PURE__*/React.createElement("button", {
      key: tab.value,
      role: "tab",
      "aria-selected": on,
      onClick: () => onChange && onChange(tab.value),
      style: {
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-6)",
        background: "none",
        border: "none",
        cursor: "pointer",
        padding: variant === "underline" ? "0 0 8px" : "4px 10px",
        borderRadius: variant === "underline" ? 0 : "var(--radius-5)",
        boxShadow: on && variant === "underline" ? "inset 0 -2px 0 var(--accent)" : "none",
        background: on && variant === "pill" ? "var(--surface-active)" : "none",
        fontFamily: "var(--font-ui)",
        fontWeight: "var(--fw-semibold)",
        lineHeight: "var(--lh-snug)",
        fontSize: "var(--fs-12)",
        color: on ? "var(--text-primary)" : "var(--text-muted)",
        transition: "var(--transition-control)"
      }
    }, tab.icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
      name: tab.icon,
      size: 13
    }) : null, tab.label, tab.count != null ? /*#__PURE__*/React.createElement("span", {
      style: {
        fontFamily: "var(--font-mono)",
        fontWeight: "var(--fw-regular)",
        lineHeight: "var(--lh-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, tab.count) : null);
  }));
}
Object.assign(__ds_scope, { Tabs });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tabs.jsx", error: String((e && e.message) || e) }); }

// components/core/Tag.jsx
try { (() => {
function Tag({
  children,
  color,
  onRemove,
  mono = true,
  style
}) {
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: "var(--space-6)",
      padding: "2px 4px 2px 6px",
      background: "var(--surface-active)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-5)",
      font: mono ? "var(--type-code)" : "var(--type-label)",
      color: "var(--text-body)",
      ...style
    }
  }, color ? /*#__PURE__*/React.createElement("span", {
    style: {
      width: 7,
      height: 7,
      borderRadius: 2,
      background: color,
      flex: "none"
    }
  }) : null, children, onRemove ? /*#__PURE__*/React.createElement("button", {
    onClick: onRemove,
    "aria-label": "Remove",
    style: {
      display: "grid",
      placeItems: "center",
      width: 14,
      height: 14,
      border: "none",
      background: "none",
      color: "var(--text-muted)",
      cursor: "pointer",
      padding: 0
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "x",
    size: 10
  })) : null);
}
Object.assign(__ds_scope, { Tag });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tag.jsx", error: String((e && e.message) || e) }); }

// components/core/Tooltip.jsx
try { (() => {
function Tooltip({
  content,
  children,
  side = "top",
  mono = false,
  style
}) {
  const [show, setShow] = React.useState(false);
  const pos = {
    top: {
      bottom: "calc(100% + 6px)",
      left: "50%",
      transform: "translateX(-50%)"
    },
    bottom: {
      top: "calc(100% + 6px)",
      left: "50%",
      transform: "translateX(-50%)"
    },
    left: {
      right: "calc(100% + 6px)",
      top: "50%",
      transform: "translateY(-50%)"
    },
    right: {
      left: "calc(100% + 6px)",
      top: "50%",
      transform: "translateY(-50%)"
    }
  }[side];
  return /*#__PURE__*/React.createElement("span", {
    style: {
      position: "relative",
      display: "inline-flex",
      ...style
    },
    onMouseEnter: () => setShow(true),
    onMouseLeave: () => setShow(false)
  }, children, show ? /*#__PURE__*/React.createElement("span", {
    role: "tooltip",
    style: {
      position: "absolute",
      zIndex: 50,
      ...pos,
      maxWidth: 260,
      width: "max-content",
      background: "var(--slate-800)",
      color: "var(--text-primary)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-5)",
      boxShadow: "var(--shadow-overlay)",
      padding: "6px 8px",
      font: mono ? "var(--type-code)" : "var(--type-label)",
      whiteSpace: "pre-wrap",
      pointerEvents: "none"
    }
  }, content) : null);
}
Object.assign(__ds_scope, { Tooltip });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Tooltip.jsx", error: String((e && e.message) || e) }); }

// components/data/CodeBlock.jsx
try { (() => {
const TOKENS = {
  kw: "var(--syn-keyword)",
  ty: "var(--syn-type)",
  str: "var(--syn-string)",
  num: "var(--syn-number)",
  fn: "var(--syn-fn)",
  macro: "var(--syn-macro)",
  cm: "var(--syn-comment)",
  punct: "var(--syn-punct)",
  mnem: "var(--syn-mnemonic)",
  reg: "var(--syn-register)",
  addr: "var(--syn-addr)",
  plain: "var(--syn-plain)"
};

/** Lines are pre-tokenized by the caller: string, or an array of [kind, text] pairs.
    No highlighter is bundled — the real app uses CodeMirror 6. */
function CodeBlock({
  lines = [],
  startLine = 1,
  highlightLines = [],
  gutter = true,
  annotations = {},
  style
}) {
  return /*#__PURE__*/React.createElement("pre", {
    style: {
      margin: 0,
      background: "var(--surface-code)",
      font: "var(--type-code)",
      color: "var(--syn-plain)",
      overflow: "auto",
      padding: "var(--space-8) 0",
      ...style
    }
  }, lines.map((ln, i) => {
    const n = startLine + i;
    const hot = highlightLines.includes(n);
    const parts = typeof ln === "string" ? [["plain", ln]] : ln;
    return /*#__PURE__*/React.createElement("div", {
      key: n,
      style: {
        display: "flex",
        background: hot ? "var(--syn-line-highlight)" : "transparent",
        boxShadow: hot ? "inset 2px 0 0 var(--accent)" : "none",
        minHeight: 19
      }
    }, gutter ? /*#__PURE__*/React.createElement("span", {
      style: {
        flex: "none",
        width: 48,
        textAlign: "right",
        paddingRight: 12,
        color: "var(--syn-gutter)",
        userSelect: "none"
      }
    }, n) : null, /*#__PURE__*/React.createElement("span", {
      style: {
        whiteSpace: "pre",
        paddingRight: 12
      }
    }, parts.map((p, j) => /*#__PURE__*/React.createElement("span", {
      key: j,
      style: {
        color: TOKENS[p[0]] || TOKENS.plain
      }
    }, p[1])), annotations[n] ? /*#__PURE__*/React.createElement("span", {
      style: {
        color: "var(--text-accent)",
        background: "var(--accent-quiet)",
        borderRadius: "var(--radius-2)",
        padding: "0 4px",
        marginLeft: 10
      }
    }, annotations[n]) : null));
  }));
}
Object.assign(__ds_scope, { CodeBlock });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/CodeBlock.jsx", error: String((e && e.message) || e) }); }

// components/data/DataTable.jsx
try { (() => {
/** Dense virtual-less table for symbol lists, sweep results, gate results.
    Real Binmap virtualizes tens of thousands of rows; this is the cosmetic shape. */
function DataTable({
  columns = [],
  rows = [],
  selectedIndex,
  onSelect,
  dense = true,
  zebra = false,
  style
}) {
  const [hover, setHover] = React.useState(-1);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      overflow: "auto",
      minHeight: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("table", {
    style: {
      width: "100%",
      borderCollapse: "collapse",
      fontVariantNumeric: "tabular-nums"
    }
  }, /*#__PURE__*/React.createElement("thead", null, /*#__PURE__*/React.createElement("tr", null, columns.map(c => /*#__PURE__*/React.createElement("th", {
    key: c.key,
    style: {
      position: "sticky",
      top: 0,
      zIndex: 1,
      textAlign: c.align || "left",
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      background: "var(--surface-raised)",
      padding: "0 10px",
      height: 26,
      whiteSpace: "nowrap",
      borderBottom: "1px solid var(--border-default)",
      width: c.width
    }
  }, c.label)))), /*#__PURE__*/React.createElement("tbody", null, rows.map((r, i) => {
    const on = i === selectedIndex;
    return /*#__PURE__*/React.createElement("tr", {
      key: r.id || i,
      onClick: () => onSelect && onSelect(r, i),
      onMouseEnter: () => setHover(i),
      onMouseLeave: () => setHover(-1),
      style: {
        height: dense ? "var(--row-h)" : "var(--row-h-lg)",
        background: on ? "var(--surface-selected)" : hover === i ? "var(--surface-hover)" : zebra && i % 2 ? "rgba(255,255,255,.012)" : "transparent",
        boxShadow: on ? "inset 2px 0 0 var(--accent)" : "none",
        cursor: onSelect ? "pointer" : "default",
        transition: "background-color var(--dur-instant) var(--ease-standard)"
      }
    }, columns.map(c => /*#__PURE__*/React.createElement("td", {
      key: c.key,
      style: {
        padding: "0 10px",
        textAlign: c.align || "left",
        borderBottom: "1px solid var(--border-subtle)",
        fontFamily: c.mono === false ? "var(--font-ui)" : "var(--font-mono)",
        fontSize: "var(--fs-12)",
        color: c.muted ? "var(--text-muted)" : "var(--text-body)",
        whiteSpace: "nowrap",
        overflow: "hidden",
        textOverflow: "ellipsis",
        maxWidth: c.maxWidth
      }
    }, c.render ? c.render(r) : r[c.key])));
  }), rows.length === 0 ? /*#__PURE__*/React.createElement("tr", null, /*#__PURE__*/React.createElement("td", {
    colSpan: columns.length,
    style: {
      padding: "var(--space-24)",
      textAlign: "center",
      color: "var(--text-muted)",
      font: "var(--type-body)"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "inbox",
    size: 16,
    style: {
      marginRight: 6,
      verticalAlign: "-3px"
    }
  }), "No rows")) : null)));
}
Object.assign(__ds_scope, { DataTable });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/DataTable.jsx", error: String((e && e.message) || e) }); }

// components/data/DeltaValue.jsx
try { (() => {
/** Signed metric change. Direction of "good" is explicit — smaller is better for size,
    larger is better for throughput — so the caller states it rather than us guessing. */
function DeltaValue({
  value,
  unit = "%",
  lowerIsBetter = true,
  significant = true,
  showIcon = true,
  size = "md",
  style
}) {
  const flat = value === 0 || !significant;
  const good = lowerIsBetter ? value < 0 : value > 0;
  const color = flat ? "var(--delta-flat)" : good ? "var(--delta-improve)" : "var(--delta-regress)";
  const sign = value > 0 ? "+" : value < 0 ? "\u2212" : "\u00b1";
  const fs = size === "lg" ? "var(--fs-22)" : size === "sm" ? "var(--fs-11)" : "var(--fs-13)";
  return /*#__PURE__*/React.createElement("span", {
    style: {
      display: "inline-flex",
      alignItems: "center",
      gap: 3,
      color,
      fontFamily: "var(--font-mono)",
      fontSize: fs,
      fontWeight: "var(--fw-medium)",
      fontVariantNumeric: "tabular-nums",
      ...style
    }
  }, showIcon && !flat ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: value > 0 ? "arrow-up" : "arrow-down",
    size: size === "lg" ? 16 : 11
  }) : null, sign, Math.abs(value), unit, !significant ? /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      marginLeft: 3
    }
  }, "n.s.") : null);
}
Object.assign(__ds_scope, { DeltaValue });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/DeltaValue.jsx", error: String((e && e.message) || e) }); }

// components/data/MetricStat.jsx
try { (() => {
function MetricStat({
  label,
  value,
  unit,
  delta,
  lowerIsBetter = true,
  significant = true,
  provenance,
  footnote,
  onClick,
  style
}) {
  const [hover, setHover] = React.useState(false);
  const marker = provenance === "measured" ? ["\u25CF", "var(--prov-measured)"] : provenance === "derived" ? ["\u25C8", "var(--prov-derived)"] : provenance === "inferred" ? ["\u25C6", "var(--prov-inferred)"] : null;
  return /*#__PURE__*/React.createElement("div", {
    onClick: onClick,
    onMouseEnter: () => setHover(true),
    onMouseLeave: () => setHover(false),
    style: {
      padding: "var(--space-12)",
      background: hover && onClick ? "var(--surface-hover)" : "var(--surface-panel)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-8)",
      cursor: onClick ? "pointer" : "default",
      transition: "var(--transition-surface)",
      minWidth: 0,
      ...style
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: "var(--space-6)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, label), marker ? /*#__PURE__*/React.createElement("span", {
    title: provenance,
    style: {
      color: marker[1],
      fontSize: 9,
      lineHeight: 1
    }
  }, marker[0]) : null, onClick ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "chevron-right",
    size: 12,
    color: "var(--text-muted)",
    style: {
      marginLeft: "auto"
    }
  }) : null), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "baseline",
      flexWrap: "wrap",
      rowGap: 2,
      gap: "var(--space-6)",
      marginTop: 6,
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontFamily: "var(--font-mono)",
      fontSize: "var(--fs-22)",
      fontWeight: "var(--fw-medium)",
      color: "var(--text-primary)",
      fontVariantNumeric: "tabular-nums"
    }
  }, value), unit ? /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      color: "var(--text-muted)"
    }
  }, unit) : null, delta != null ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(__ds_scope.DeltaValue, {
    value: delta,
    lowerIsBetter: lowerIsBetter,
    significant: significant
  })) : null), footnote ? /*#__PURE__*/React.createElement("div", {
    style: {
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)",
      marginTop: 6
    }
  }, footnote) : null);
}
Object.assign(__ds_scope, { MetricStat });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/MetricStat.jsx", error: String((e && e.message) || e) }); }

// components/data/ProgressBar.jsx
try { (() => {
function ProgressBar({
  value = 0,
  max = 100,
  tone = "accent",
  label,
  sublabel,
  indeterminate = false,
  height = 6,
  style
}) {
  const pct = Math.max(0, Math.min(100, value / max * 100));
  const color = tone === "pass" ? "var(--status-pass)" : tone === "fail" ? "var(--status-fail)" : tone === "neutral" ? "var(--slate-400)" : "var(--accent)";
  return /*#__PURE__*/React.createElement("div", {
    style: {
      minWidth: 0,
      ...style
    }
  }, label || sublabel ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--space-8)",
      marginBottom: 6,
      alignItems: "baseline"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-label)",
      color: "var(--text-body)"
    }
  }, label), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      font: "var(--type-code)",
      color: "var(--text-muted)"
    }
  }, sublabel)) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      height,
      background: "var(--surface-active)",
      borderRadius: "var(--radius-full)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      height: "100%",
      width: indeterminate ? "35%" : pct + "%",
      background: color,
      borderRadius: "var(--radius-full)",
      transition: "width var(--dur-base) var(--ease-standard)",
      animation: indeterminate ? "sk-indeterminate 1.2s var(--ease-standard) infinite" : undefined
    }
  })));
}
Object.assign(__ds_scope, { ProgressBar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/data/ProgressBar.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-cli/Sessions.jsx
try { (() => {
const {
  Badge,
  SegmentedControl,
  Button
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function CliSurface() {
  const [cmd, setCmd] = React.useState("tune");
  const p = (color, text, bold) => [color, text, bold];
  const tune = [[p(C.dim, "$ "), p(C.strong, "binmap tune", true)], [p(C.muted, "  project   "), p(C.body, "my-crate 0.4.1  ·  a3f9c21 (dirty)")], [p(C.muted, "  baseline  "), p(C.body, "release  1,966,208 B  381.4 ms")], [], [p(C.accent, "  sweeping 36 configurations"), p(C.muted, "  (cache hit 11)")], [p(C.pass, "  \u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588"), p(C.dim, "\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591"), p(C.muted, "  17/36  2m14s")], [], [p(C.strong, "  pareto frontier", true)], [p(C.muted, "  #  flags                                  size        \u0394        runtime    build")], [p(C.accent, "  1  opt-level=z lto=fat cgu=1 panic=abort  1,214,336  "), p(C.pass, "-38.2%"), p(C.body, "   412.7 ms  2m04s")], [p(C.body, "  2  opt-level=s lto=thin cgu=16             1,489,120  "), p(C.pass, "-24.4%"), p(C.body, "   388.1 ms  1m41s")], [p(C.body, "  3  opt-level=3 lto=off cgu=16 (default)    1,966,208  "), p(C.muted, "  0.0%"), p(C.body, "   381.4 ms  1m12s")], [], [p(C.fail, "  rejected  "), p(C.body, "opt-level=z build-std panic_immediate_abort")], [p(C.muted, "            gate TestsPass failed: 3 tests panicked (catch_unwind)")], [], [p(C.pass, "  \u2713 "), p(C.body, "tests pass  ·  benchmark +1.1% "), p(C.muted, "(n.s., noise floor 0.9%)")], [p(C.pass, "  \u2713 "), p(C.body, "best configuration written to "), p(C.strong, "binmap.json")], [], [p(C.muted, "  apply with: "), p(C.accent, "binmap tune --apply")], [p(C.muted, "  trust tier is Propose; --apply requires Tune")]];
  const size = [[p(C.dim, "$ "), p(C.strong, "binmap size --no-ai", true)], [p(C.muted, "  target/release/my-crate  1,966,208 B  (.text 1,182,208)")], [], [p(C.strong, "  where the bytes went", true)], [p(C.body, "  412 KB  20.9%  my_crate")], [p(C.body, "  214 KB  10.9%  core::fmt          "), p(C.muted, "41 call sites, 18 in panics")], [p(C.body, "  186 KB   9.5%  serde_json")], [p(C.body, "  148 KB   7.5%  .eh_frame          "), p(C.muted, "unwinding tables")], [p(C.body, "   96 KB   4.9%  [panic strings]")], [p(C.body, "   64 KB   3.3%  Drop glue          "), p(C.muted, "31 instantiations")], [], [p(C.strong, "  monomorphization groups", true)], [p(C.body, "  serde_json::de::from_str<T>   12 \u00d7  17,906 avg   214,880 total")], [p(C.body, "  <&T as Display>::fmt          18 \u00d7     480 avg     8,640 total")], [], [p(C.muted, "  4 findings  ·  3 measured, 1 derived  ·  0 model claims (--no-ai)")], [p(C.muted, "  artifact: binmap.json")]];
  const analyze = [[p(C.dim, "$ "), p(C.strong, "binmap analyze core.1234", true)], [p(C.pass, "  \u2713 "), p(C.body, "binary \u2194 source verified  "), p(C.muted, "(build id matches)")], [], [p(C.fail, "  IndexOutOfBounds"), p(C.body, "  src/parser.rs:142  0x00401f2a")], [p(C.muted, "    141 |     debug_assert!(idx <= buf.len());")], [p(C.accent, "  > 142 |     buf[idx]"), p(C.muted, "                          idx = 4096, len = 4096")], [], [p(C.strong, "  stack", true), p(C.muted, "  (2 frames inlined)")], [p(C.body, "   #0  my_crate::parse::Parser::index         parser.rs:142")], [p(C.muted, "   #1    \u21b3 core::slice::index::panic_bounds_check  index.rs:36  [inlined]")], [p(C.body, "   #2  core::panicking::panic_fmt             panicking.rs:72")], [p(C.body, "   #3  my_crate::parse::Parser::run           parser.rs:61")], [], [p(C.strong, "  root cause", true), p(C.accent, "  [probable \u25c6 inferred, cites 4]")], [p(C.body, "  Parser::run passes the window length instead of the last valid")], [p(C.body, "  offset. The debug_assert at parser.rs:140 is compiled out at")], [p(C.body, "  opt-level=3, so the bound is never checked in release.")], [], [p(C.muted, "  evidence: addr2line 0.24, lldb 18.1, gimli 0.31, register_recovery")]];
  const ci = [[p(C.dim, "$ "), p(C.strong, "binmap ci --baseline .binmap/baseline.json", true)], [], [p(C.pass, "  \u2713 size      1,214,336 B   -38.2%   budget 1,400,000 B")], [p(C.warn, "  ! runtime    412.7 ms     +1.1%    n.s. (noise floor 0.9%)")], [p(C.fail, "  \u2717 .eh_frame   148,000 B   +12.4%   budget 140,000 B")], [], [p(C.fail, "  1 budget exceeded"), p(C.body, "  \u2192 exit 1")], [p(C.muted, "  report: binmap.json  ·  annotate PR with --format github")]];
  const map = {
    tune,
    size,
    analyze,
    ci
  };
  const titles = {
    tune: "binmap tune",
    size: "binmap size --no-ai",
    analyze: "binmap analyze core.1234",
    ci: "binmap ci"
  };
  return /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 16
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 12
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-13)",
      color: "var(--text-primary)"
    }
  }, "binmap"), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, "command-line surface"), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(SegmentedControl, {
    value: cmd,
    onChange: setCmd,
    options: [{
      value: "tune",
      label: "tune"
    }, {
      value: "size",
      label: "size"
    }, {
      value: "analyze",
      label: "analyze"
    }, {
      value: "ci",
      label: "ci"
    }]
  }))), /*#__PURE__*/React.createElement(Terminal, {
    title: titles[cmd] + "  ·  linux x86-64",
    lines: map[cmd],
    footer: /*#__PURE__*/React.createElement(Badge, {
      mono: true,
      tone: cmd === "ci" ? "fail" : "pass"
    }, cmd === "ci" ? "exit 1" : "exit 0")
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-body)",
      fontSize: "var(--fs-12)",
      color: "var(--text-muted)"
    }
  }, "Colour in the CLI carries the same meaning as in the GUI: green measured and passing, orange for accent and model claims, red for failed gates, grey for anything below the noise floor. Model output is always prefixed with its confidence and the \u25C6 inferred marker."));
}
Object.assign(window, {
  CliSurface
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-cli/Sessions.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-cli/Terminal.jsx
try { (() => {
const {
  Badge,
  SegmentedControl,
  Icon,
  ProvenanceBadge
} = window.SilverKeyLodestoneDesignSystem_2b7558;

/* The CLI ships first in every phase (PRD §10.2), so it is a real brand surface.
   Colour is the only styling a terminal has: the same tokens, mapped to ANSI. */
function Line({
  parts
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      whiteSpace: "pre-wrap",
      minHeight: 18
    }
  }, parts.map((p, i) => /*#__PURE__*/React.createElement("span", {
    key: i,
    style: {
      color: p[0],
      fontWeight: p[2] ? 600 : 400
    }
  }, p[1])));
}
const C = {
  dim: "var(--text-disabled)",
  body: "var(--text-body)",
  strong: "var(--text-primary)",
  accent: "var(--orange-400)",
  pass: "var(--status-pass)",
  fail: "var(--status-fail)",
  warn: "var(--status-warn)",
  info: "var(--status-info)",
  muted: "var(--text-muted)"
};
function Terminal({
  title,
  lines,
  footer
}) {
  return /*#__PURE__*/React.createElement("div", {
    style: {
      background: "var(--surface-code)",
      border: "1px solid var(--border-default)",
      borderRadius: "var(--radius-8)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 8,
      height: 30,
      padding: "0 12px",
      background: "var(--surface-raised)",
      borderBottom: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "terminal",
    size: 13,
    color: "var(--text-muted)"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, title), footer ? /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, footer) : null), /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "12px 14px",
      font: "var(--type-code-lg)",
      color: "var(--text-body)",
      overflow: "auto"
    }
  }, lines.map((l, i) => /*#__PURE__*/React.createElement(Line, {
    key: i,
    parts: l
  }))));
}
Object.assign(window, {
  Terminal,
  Line,
  C
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-cli/Terminal.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/AgentSetup.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Badge,
  Tag,
  ProviderCard,
  AcpAgentCard,
  ModelPicker,
  ApiKeyField,
  TrustTierControl,
  ProgressBar
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function AgentSetup() {
  const M = window.LODE_MODELS;
  const [tab, setTab] = React.useState("agents");
  const [agent, setAgent] = React.useState("Claude Agent");
  const [provider, setProvider] = React.useState("Anthropic");
  const [model, setModel] = React.useState("claude-opus-5");
  const [filter, setFilter] = React.useState("");
  const [customUrl, setCustomUrl] = React.useState("http://10.0.0.4:8000/v1");
  const [noAi, setNoAi] = React.useState(false);
  const [tested, setTested] = React.useState({});
  const [keys, setKeys] = React.useState({});
  const groups = M.providers.filter(p => p.models.length).map(p => ({
    provider: p.provider,
    transport: p.transport,
    connected: p.connected,
    models: p.models
  }));
  const sel = M.providers.find(p => p.provider === provider);
  const test = name => {
    setTested(t => ({
      ...t,
      [name]: "testing"
    }));
    setTimeout(() => setTested(t => ({
      ...t,
      [name]: "valid"
    })), 900);
  };
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)",
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 12,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(Tabs, {
    value: tab,
    onChange: setTab,
    tabs: [{
      value: "agents",
      label: "Coding agents",
      count: M.acpAgents.length
    }, {
      value: "providers",
      label: "Model providers",
      count: M.providers.length
    }, {
      value: "model",
      label: "Analysis model"
    }]
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto",
      display: "flex",
      alignItems: "center",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement(Switch, {
    checked: !noAi,
    onChange: v => setNoAi(!v),
    label: "Model layer",
    size: "sm"
  }), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: noAi ? "var(--status-warn)" : "var(--text-muted)"
    }
  }, noAi ? "--no-ai · deterministic analysis only" : "model claims capped at Probable"))), tab === "agents" ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Agents on the Agent Client Protocol",
    icon: "waypoints",
    scroll: true,
    style: {
      flex: 1,
      minWidth: 0
    },
    actions: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(Badge, {
      mono: true,
      tone: "info"
    }, "JSON-RPC 2.0 \xB7 stdio"), /*#__PURE__*/React.createElement(Button, {
      size: "sm",
      variant: "secondary",
      icon: "plus"
    }, "Add agent"))
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, M.acpAgents.map(a => /*#__PURE__*/React.createElement(AcpAgentCard, _extends({
    key: a.name
  }, a, {
    selected: agent === a.name,
    onSelect: () => setAgent(a.name),
    onConnect: () => {},
    onLogin: () => {}
  }))))), /*#__PURE__*/React.createElement(Panel, {
    title: "How agents are wired",
    icon: "circle-help",
    scroll: true,
    style: {
      width: 360,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 12,
      font: "var(--type-body)",
      color: "var(--text-secondary)"
    }
  }, /*#__PURE__*/React.createElement("p", {
    style: {
      margin: 0
    }
  }, "An external agent runs as its own process and speaks ACP to Binmap over stdin and stdout. Binmap is the client: it exposes the project's files, the terminal, and its own analysis tools, and it holds the permission boundary."), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, "What Binmap offers an agent"), [["fs.read / fs.write", "source, Cargo.toml, binmap.json"], ["terminal", "cargo, hyperfine, perf — inside the trust tier"], ["binmap.tools", "the deterministic analysis tools, as MCP-style calls"], ["permissions", "every write and every command is confirmable"]].map(([k, v]) => /*#__PURE__*/React.createElement("div", {
    key: k,
    style: {
      display: "flex",
      gap: 10,
      font: "var(--type-code)",
      fontSize: "var(--fs-11)"
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 132,
      color: "var(--text-accent)"
    }
  }, k), /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--text-muted)",
      flex: 1
    }
  }, v)))), /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "8px 10px",
      background: "var(--accent-quiet)",
      border: "1px solid rgba(255,111,21,.22)",
      borderRadius: "var(--radius-5)",
      fontSize: "var(--fs-11)"
    }
  }, "An external agent authenticates itself. Keys you store under Model providers configure Binmap's own analysis model, not the agent."), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, "Trust tier applies to agents too"), /*#__PURE__*/React.createElement(TrustTierControl, {
    variant: "list",
    value: "propose"
  }))))) : tab === "providers" ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Providers",
    icon: "cloud",
    scroll: true,
    style: {
      flex: 1,
      minWidth: 0
    },
    actions: /*#__PURE__*/React.createElement(Button, {
      size: "sm",
      variant: "secondary",
      icon: "plus"
    }, "Add OpenAI-compatible")
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, M.providers.map(p => /*#__PURE__*/React.createElement(ProviderCard, {
    key: p.provider,
    name: p.provider,
    kind: p.kind,
    baseUrl: p.custom ? customUrl : "https://" + p.transport.split(" ")[0],
    envVar: p.env === "none" ? null : p.env,
    keyStored: !!p.keyFingerprint,
    keyFingerprint: p.keyFingerprint,
    status: tested[p.provider] || p.status || "idle",
    modelCount: p.models.length,
    docsHint: p.hint,
    apiKey: keys[p.provider] || "",
    onApiKeyChange: e => setKeys(k => ({
      ...k,
      [p.provider]: e.target.value
    })),
    selected: provider === p.provider,
    onSelect: () => setProvider(p.provider),
    onTest: () => test(p.provider),
    editableBaseUrl: !!p.custom,
    onBaseUrlChange: e => setCustomUrl(e.target.value),
    footer: p.custom ? /*#__PURE__*/React.createElement("div", {
      style: {
        display: "grid",
        gap: 8
      }
    }, /*#__PURE__*/React.createElement("div", {
      style: {
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        gap: 8
      }
    }, /*#__PURE__*/React.createElement(Input, {
      size: "sm",
      mono: true,
      placeholder: "model id, e.g. qwen3-coder-next",
      icon: "box"
    }), /*#__PURE__*/React.createElement(Select, {
      size: "sm",
      options: [{
        value: "chat",
        label: "/v1/chat/completions"
      }, {
        value: "resp",
        label: "/v1/responses"
      }]
    })), /*#__PURE__*/React.createElement("div", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "models discovered from /v1/models when the key is saved")) : null
  })))), /*#__PURE__*/React.createElement(Panel, {
    title: sel ? sel.provider : "Provider",
    icon: "key-round",
    scroll: true,
    style: {
      width: 360,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 12
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6,
      font: "var(--type-code)",
      fontSize: "var(--fs-11)"
    }
  }, [["transport", sel && sel.transport], ["auth", sel && sel.env === "none" ? "none" : sel && sel.env], ["models", sel && sel.models.length ? sel.models.length + " listed" : "discovered at runtime"], ["wire format", sel && sel.kind === "native" ? "first-party SDK" : sel && sel.kind === "local" ? "OpenAI-compatible (local)" : "OpenAI-compatible"]].map(([k, v]) => /*#__PURE__*/React.createElement("div", {
    key: k,
    style: {
      display: "flex",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 92,
      color: "var(--text-muted)"
    }
  }, k), /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--text-primary)",
      flex: 1,
      wordBreak: "break-all"
    }
  }, v)))), sel && sel.models.length ? /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      marginBottom: 6
    }
  }, "Supported models \xB7 newest first"), /*#__PURE__*/React.createElement("div", {
    style: {
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-5)",
      overflow: "hidden"
    }
  }, /*#__PURE__*/React.createElement(ModelPicker, {
    height: 260,
    value: model,
    onChange: setModel,
    groups: [{
      provider: sel.provider,
      transport: sel.transport,
      connected: sel.connected,
      models: sel.models
    }]
  }))) : null, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-body)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "Retired models are not offered: kimi-k2.5 and moonshot-v1 (31 Aug 2026), Claude 3.x, GPT-4.x.")))) : /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Analysis model",
    icon: "box",
    padding: false,
    style: {
      flex: 1,
      minWidth: 0
    },
    actions: /*#__PURE__*/React.createElement(Badge, {
      mono: true,
      tone: "neutral"
    }, groups.reduce((n, g) => n + g.models.length, 0), " models \xB7 ", groups.length, " providers"),
    footer: /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "selected: ", model, " \xB7 binmap.toml [model] backend")
  }, /*#__PURE__*/React.createElement(ModelPicker, {
    groups: groups,
    value: model,
    onChange: setModel,
    filter: filter,
    onFilterChange: setFilter,
    height: "100%",
    style: {
      height: "100%"
    }
  })), /*#__PURE__*/React.createElement(Panel, {
    title: "Budgets and behaviour",
    icon: "gauge",
    scroll: true,
    style: {
      width: 360,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 14
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement(ProgressBar, {
    value: 25,
    max: 25,
    label: "Steps per session",
    sublabel: "25",
    tone: "neutral"
  }), /*#__PURE__*/React.createElement(ProgressBar, {
    value: 64,
    max: 128,
    label: "Token budget",
    sublabel: "64k of 128k",
    tone: "neutral"
  }), /*#__PURE__*/React.createElement(ProgressBar, {
    value: 10,
    max: 30,
    label: "Wall-clock cap",
    sublabel: "10s per step",
    tone: "neutral"
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 8
    }
  }, /*#__PURE__*/React.createElement(Switch, {
    checked: true,
    label: "Require evidence citations",
    size: "sm"
  }), /*#__PURE__*/React.createElement(Switch, {
    checked: true,
    label: "Cap model claims at Probable",
    size: "sm"
  }), /*#__PURE__*/React.createElement(Switch, {
    label: "Allow cloud providers for this project",
    size: "sm"
  }), /*#__PURE__*/React.createElement(Switch, {
    checked: true,
    label: "Redact paths and identifiers on export",
    size: "sm"
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, "Fallback order"), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: 6,
      flexWrap: "wrap"
    }
  }, /*#__PURE__*/React.createElement(Tag, {
    color: "var(--cat-yours)"
  }, "local/qwen3-coder-next"), /*#__PURE__*/React.createElement(Tag, {
    color: "var(--cat-deps)"
  }, "anthropic/claude-opus-5"), /*#__PURE__*/React.createElement(Tag, {
    color: "var(--cat-fmt)"
  }, "deepseek/v4-pro")), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "first reachable backend wins; --no-ai skips all of them"))))));
}
Object.assign(window, {
  AgentSetup
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/AgentSetup.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/AgentTranscript.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function AgentTranscript() {
  const L = window.LODE;
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)",
      overflow: "auto"
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Agent session \xB7 size attribution",
    icon: "waypoints",
    style: {
      flex: "none"
    },
    actions: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(Badge, {
      mono: true,
      tone: "info"
    }, "local/qwen3-coder-next"), /*#__PURE__*/React.createElement(Button, {
      size: "sm",
      variant: "secondary",
      icon: "download"
    }, "Export session"))
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "repeat(3,1fr)",
      gap: 12,
      marginBottom: 12
    }
  }, /*#__PURE__*/React.createElement(ProgressBar, {
    value: 4,
    max: 25,
    label: "Step budget",
    sublabel: "4/25"
  }), /*#__PURE__*/React.createElement(ProgressBar, {
    value: 9300,
    max: 64000,
    label: "Tokens",
    sublabel: "9.3k/64k",
    tone: "neutral"
  }), /*#__PURE__*/React.createElement(ProgressBar, {
    value: 2,
    max: 10,
    label: "Wall time",
    sublabel: "2.0s/10s",
    tone: "neutral"
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 8
    }
  }, L.transcript.map(t => /*#__PURE__*/React.createElement(TranscriptStep, _extends({
    key: t.step
  }, t, {
    defaultOpen: t.step === 3
  }))))), /*#__PURE__*/React.createElement(Panel, {
    title: "Conclusion",
    icon: "flag",
    style: {
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: 6
    }
  }, /*#__PURE__*/React.createElement(ProvenanceBadge, {
    kind: "inferred"
  }), /*#__PURE__*/React.createElement(ConfidenceBadge, {
    level: "probable",
    showNote: true
  })), /*#__PURE__*/React.createElement("p", {
    style: {
      margin: 0,
      font: "var(--type-body)",
      color: "var(--text-body)"
    }
  }, "Twelve instantiations of ", /*#__PURE__*/React.createElement("code", null, "serde_json::de::from_str"), " account for 214,880 bytes. Eleven differ only in their error type and are reachable only from ", /*#__PURE__*/React.createElement("code", null, "Config::load"), ", whose callers discard the concrete error. Collapsing them behind ", /*#__PURE__*/React.createElement("code", null, "Box<dyn Error>"), " should recover roughly 214 KB."), /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "Cites 4 evidence items \xB7 21 steps of budget remaining \xB7 next: verify the patch under the full gate set")))));
}
Object.assign(window, {
  AgentTranscript
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/AgentTranscript.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/AppFrame.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function AppFrame() {
  const L = window.LODE;
  const [view, setView] = React.useState("project");
  const [palette, setPalette] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [tier, setTier] = React.useState("propose");
  const [tierDialog, setTierDialog] = React.useState(false);
  const [run, setRun] = React.useState({
    status: "idle",
    message: "Ready",
    progress: null,
    best: null,
    elapsed: null
  });
  React.useEffect(() => {
    const onKey = e => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette(p => !p);
      }
      if (e.key === "Escape") setPalette(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const startRun = kind => {
    const target = kind === "tune" ? "tune" : kind;
    setRun({
      status: "running",
      message: kind === "tune" ? "Sweeping configurations" : "Measuring",
      progress: kind === "tune" ? "3/36" : "reading DWARF",
      best: null,
      elapsed: "0m 04s"
    });
    setView(target);
    setTimeout(() => setRun({
      status: "running",
      message: kind === "tune" ? "Sweeping configurations" : "Attributing bytes",
      progress: kind === "tune" ? "17/36" : "symbolizing",
      best: kind === "tune" ? "1.19 MB (−38%)" : null,
      elapsed: "2m 14s"
    }), 900);
    setTimeout(() => setRun({
      status: "done",
      message: kind === "tune" ? "Sweep complete · 36 configurations" : "Analysis complete",
      progress: null,
      best: kind === "tune" ? "1.19 MB (−38%)" : "−38.2%",
      elapsed: "3m 41s"
    }), 2600);
  };
  const views = {
    project: /*#__PURE__*/React.createElement(ProjectView, {
      onRun: startRun
    }),
    size: /*#__PURE__*/React.createElement(SizeExplorer, null),
    tune: /*#__PURE__*/React.createElement(ProfileLab, null),
    crash: /*#__PURE__*/React.createElement(CrashAnalysis, null),
    perf: /*#__PURE__*/React.createElement(PerfLab, null),
    agent: /*#__PURE__*/React.createElement(AgentTranscript, null),
    setup: /*#__PURE__*/React.createElement(AgentSetup, null)
  };
  return /*#__PURE__*/React.createElement("div", {
    style: {
      position: "relative",
      display: "flex",
      flexDirection: "column",
      height: "100%",
      minHeight: 0,
      background: "var(--surface-app)"
    }
  }, /*#__PURE__*/React.createElement(TitleBar, {
    project: L.project.name,
    commit: L.project.commit,
    dirty: L.project.dirty,
    right: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(TrustTierControl, {
      value: tier,
      onToggle: () => setTierDialog(true)
    }), /*#__PURE__*/React.createElement(Select, {
      size: "sm",
      value: "local",
      options: [{
        value: "local",
        label: "local/qwen3-coder-next"
      }, {
        value: "claude",
        label: "anthropic/claude-opus-5"
      }, {
        value: "codex",
        label: "acp/codex-cli"
      }, {
        value: "none",
        label: "--no-ai"
      }],
      onChange: () => setView("setup")
    }), /*#__PURE__*/React.createElement(IconButton, {
      icon: "search",
      label: "Command palette  \u2318K",
      size: "sm",
      onClick: () => setPalette(true)
    }), /*#__PURE__*/React.createElement(IconButton, {
      icon: "settings",
      label: "Settings",
      size: "sm"
    }))
  }), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minHeight: 0,
      display: "flex"
    }
  }, /*#__PURE__*/React.createElement(NavRail, {
    value: view,
    onChange: setView,
    footer: /*#__PURE__*/React.createElement(IconButton, {
      icon: "circle-help",
      label: "Help",
      size: "sm"
    }),
    items: [{
      value: "project",
      label: "Project",
      icon: "folder-open"
    }, {
      value: "size",
      label: "Size",
      icon: "hard-drive",
      badge: "4"
    }, {
      value: "tune",
      label: "Tune",
      icon: "sliders-horizontal",
      badge: "36"
    }, {
      value: "crash",
      label: "Crash",
      icon: "bug",
      badge: "1"
    }, {
      value: "perf",
      label: "Perf",
      icon: "flame",
      badge: "2"
    }, {
      value: "agent",
      label: "Agent",
      icon: "waypoints"
    }, {
      value: "setup",
      label: "Backends",
      icon: "plug"
    }]
  }), views[view]), /*#__PURE__*/React.createElement(StatusBar, _extends({}, run, {
    onCancel: () => setRun({
      status: "idle",
      message: "Cancelled",
      progress: null,
      best: null,
      elapsed: null
    }),
    right: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("span", null, "binmap.json \xB7 4 findings"), /*#__PURE__*/React.createElement("span", null, "\u2318K"))
  })), /*#__PURE__*/React.createElement(CommandPalette, {
    open: palette,
    query: query,
    onQueryChange: setQuery,
    activeIndex: 0,
    onClose: () => setPalette(false),
    onRun: it => {
      setPalette(false);
      if (it.view) setView(it.view);
    },
    groups: [{
      label: "Analyses",
      items: [{
        id: "tune",
        label: "Sweep build configurations",
        icon: "sliders-horizontal",
        shortcut: "⌘⇧T",
        view: "tune"
      }, {
        id: "size",
        label: "Attribute binary size",
        icon: "hard-drive",
        shortcut: "⌘⇧S",
        view: "size"
      }, {
        id: "crash",
        label: "Analyze core dump…",
        icon: "bug",
        view: "crash"
      }, {
        id: "perf",
        label: "Profile the benchmark",
        icon: "flame",
        view: "perf"
      }]
    }, {
      label: "Symbols",
      items: [{
        id: "s1",
        label: "core::fmt::Formatter::pad_integral",
        icon: "code",
        detail: "40,112 bytes",
        view: "size"
      }, {
        id: "s2",
        label: "serde_json::de::from_str::<Config>",
        icon: "code",
        detail: "31,904 bytes · 12 inst.",
        view: "size"
      }]
    }, {
      label: "Session",
      items: [{
        id: "t",
        label: "Open agent transcript",
        icon: "waypoints",
        view: "agent"
      }, {
        id: "e",
        label: "Export session with redaction pass",
        icon: "download"
      }, {
        id: "b",
        label: "Configure agents and model backends",
        icon: "plug",
        shortcut: "⌘,",
        view: "setup"
      }]
    }]
  }), /*#__PURE__*/React.createElement(Dialog, {
    open: tierDialog,
    width: 460,
    tone: "warn",
    title: "Change trust tier",
    description: "The tier is a single dial. It is always visible and never raised silently.",
    onClose: () => setTierDialog(false),
    footer: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(Button, {
      variant: "ghost",
      onClick: () => setTierDialog(false)
    }, "Cancel"), /*#__PURE__*/React.createElement(Button, {
      variant: "primary",
      onClick: () => setTierDialog(false)
    }, "Confirm tier"))
  }, /*#__PURE__*/React.createElement(TrustTierControl, {
    variant: "list",
    value: tier,
    onRequestChange: setTier
  })));
}
Object.assign(window, {
  AppFrame
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/AppFrame.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/CrashAnalysis.jsx
try { (() => {
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function CrashAnalysis() {
  const L = window.LODE;
  const [frame, setFrame] = React.useState(0);
  const [pane, setPane] = React.useState("both");
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 10,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(Badge, {
    tone: "fail",
    icon: "bug"
  }, "IndexOutOfBounds"), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      color: "var(--text-body)"
    }
  }, "core.1234"), /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, "my-crate 0.4.1 \xB7 release \xB7 debug=1 \xB7 SIGABRT"), /*#__PURE__*/React.createElement(Tooltip, {
    content: "Binary and source correspond: build id matches the core dump."
  }, /*#__PURE__*/React.createElement(Badge, {
    tone: "pass",
    icon: "shield-check"
  }, "binary \u2194 source verified")), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(SegmentedControl, {
    size: "sm",
    value: pane,
    onChange: setPane,
    options: [{
      value: "src",
      label: "Source"
    }, {
      value: "both",
      label: "Both"
    }, {
      value: "asm",
      label: "Disasm"
    }]
  }))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Stack",
    icon: "layers",
    padding: false,
    scroll: true,
    style: {
      width: 300,
      flex: "none"
    },
    footer: /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "2 of 5 frames were inlined away")
  }, /*#__PURE__*/React.createElement(StackFrameList, {
    frames: L.frames,
    selectedIndex: frame,
    onSelect: (fr, i) => setFrame(i)
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)"
    }
  }, pane !== "asm" ? /*#__PURE__*/React.createElement(Panel, {
    title: "src/parser.rs",
    icon: "file-code",
    padding: false,
    scroll: true,
    style: {
      flex: 1,
      minHeight: 0
    },
    actions: /*#__PURE__*/React.createElement(Badge, {
      tone: "accent",
      mono: true
    }, "values reconstructed")
  }, /*#__PURE__*/React.createElement(CodeBlock, {
    startLine: L.source.start,
    highlightLines: L.source.highlight,
    annotations: L.source.annotations,
    lines: L.source.lines
  })) : null, pane !== "src" ? /*#__PURE__*/React.createElement(Panel, {
    title: "Disassembly \xB7 my_crate::parse::Parser::index",
    icon: "binary",
    padding: false,
    scroll: true,
    style: {
      flex: 1,
      minHeight: 0
    },
    actions: /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "scroll-synced with source")
  }, /*#__PURE__*/React.createElement(CodeBlock, {
    startLine: 1,
    gutter: false,
    highlightLines: [4],
    lines: L.disasm.lines
  })) : null))), /*#__PURE__*/React.createElement(Inspector, {
    findings: L.findings.crash,
    selectedId: "c1",
    evidence: L.evidence,
    gates: [],
    proposal: {
      summary: "Bounds-check the caller instead of the callee",
      tier: "Propose",
      blocked: "Tier 3 required to patch source",
      diff: ["@@ -61,7 +61,7 @@ impl Parser {", "-        let byte = self.index(start + len);", "+        let byte = self.index((start + len).min(self.buffer.len() - 1));"]
    }
  }));
}
Object.assign(window, {
  CrashAnalysis
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/CrashAnalysis.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/Inspector.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;

/* The Findings Inspector is always present — the physical manifestation of
   "evidence or it didn't happen" (DESIGN §7.1). */
function Inspector({
  findings = [],
  selectedId,
  onSelect,
  evidence = {},
  gates = [],
  proposal
}) {
  const [tab, setTab] = React.useState("evidence");
  const [verifying, setVerifying] = React.useState(false);
  const [verified, setVerified] = React.useState(false);
  const sel = findings.find(f => f.id === selectedId) || findings[0];
  const ev = sel && evidence[sel.id] || [];
  const verify = () => {
    setVerifying(true);
    setTimeout(() => {
      setVerifying(false);
      setVerified(true);
      setTab("proposal");
    }, 1400);
  };
  return /*#__PURE__*/React.createElement("aside", {
    style: {
      width: "var(--chrome-inspector-w)",
      flex: "none",
      display: "flex",
      flexDirection: "column",
      minHeight: 0,
      background: "var(--surface-panel)",
      borderLeft: "1px solid var(--border-subtle)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: "none",
      padding: "var(--space-10) var(--space-12) 0"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      alignItems: "center",
      gap: 6,
      marginBottom: 8
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)"
    }
  }, "Findings"), /*#__PURE__*/React.createElement(Badge, {
    tone: "neutral",
    mono: true
  }, findings.length), /*#__PURE__*/React.createElement("span", {
    style: {
      marginLeft: "auto"
    }
  }, /*#__PURE__*/React.createElement(IconButton, {
    icon: "arrow-down-narrow-wide",
    label: "Sort by impact",
    size: "sm"
  }))), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 6,
      maxHeight: 232,
      overflow: "auto",
      paddingBottom: 4
    }
  }, findings.map(fd => /*#__PURE__*/React.createElement(FindingCard, _extends({
    key: fd.id
  }, fd, {
    selected: sel && fd.id === sel.id,
    onClick: () => onSelect && onSelect(fd.id)
  }))))), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minHeight: 0,
      display: "flex",
      flexDirection: "column",
      borderTop: "1px solid var(--border-subtle)",
      marginTop: 10
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: "none",
      padding: "8px var(--space-12) 0"
    }
  }, /*#__PURE__*/React.createElement(Tabs, {
    value: tab,
    onChange: setTab,
    tabs: [{
      value: "hypothesis",
      label: "Hypothesis"
    }, {
      value: "evidence",
      label: "Evidence",
      count: ev.length
    }, {
      value: "proposal",
      label: "Proposal"
    }]
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minHeight: 0,
      overflow: "auto",
      padding: "var(--space-10) var(--space-12)"
    }
  }, !sel ? /*#__PURE__*/React.createElement("div", {
    style: {
      color: "var(--text-muted)"
    }
  }, "Select a finding.") : tab === "hypothesis" ? /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: 6,
      alignItems: "center"
    }
  }, /*#__PURE__*/React.createElement(ProvenanceBadge, {
    kind: sel.provenance
  }), /*#__PURE__*/React.createElement(ConfidenceBadge, {
    level: sel.confidence
  })), /*#__PURE__*/React.createElement("h4", {
    style: {
      margin: 0,
      font: "var(--type-subheading)",
      color: "var(--text-primary)"
    }
  }, sel.title), /*#__PURE__*/React.createElement("p", {
    style: {
      margin: 0,
      font: "var(--type-body)",
      color: "var(--text-secondary)"
    }
  }, sel.detail), /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-code)",
      fontSize: "var(--fs-11)",
      color: "var(--text-muted)"
    }
  }, sel.location), sel.provenance === "inferred" ? /*#__PURE__*/React.createElement("div", {
    style: {
      padding: "8px 10px",
      background: "var(--prov-inferred-bg)",
      border: "1px solid rgba(255,111,21,.22)",
      borderRadius: "var(--radius-5)",
      font: "var(--type-body)",
      fontSize: "var(--fs-11)",
      color: "var(--text-secondary)"
    }
  }, "This is a model claim. It cites ", ev.length, " evidence items and is capped at Probable.") : null) : tab === "evidence" ? /*#__PURE__*/React.createElement("div", null, ev.length ? ev.map((e, i) => /*#__PURE__*/React.createElement(EvidenceItem, _extends({
    key: i
  }, e, {
    defaultOpen: i === 0
  }))) : /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-body)",
      color: "var(--text-muted)"
    }
  }, "No evidence recorded for this finding."), gates.length ? /*#__PURE__*/React.createElement("div", {
    style: {
      marginTop: 12
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      marginBottom: 6
    }
  }, "Verification"), gates.map(g => /*#__PURE__*/React.createElement(GateResult, _extends({
    key: g.gate
  }, g)))) : null) : /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, proposal ? /*#__PURE__*/React.createElement(ProposalBlock, _extends({}, proposal, {
    verifying: verifying,
    onVerify: verify,
    onApply: () => {}
  })) : /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-body)",
      color: "var(--text-muted)"
    }
  }, "No proposal for this finding."), verified ? /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      marginBottom: 6
    }
  }, "Verification run"), (gates.length ? gates : window.LODE.gates).map(g => /*#__PURE__*/React.createElement(GateResult, _extends({
    key: g.gate
  }, g)))) : null))));
}
Object.assign(window, {
  Inspector
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/Inspector.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/PerfLab.jsx
try { (() => {
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function PerfLab() {
  const L = window.LODE;
  const [fr, setFr] = React.useState("idx");
  const [sel, setSel] = React.useState("p1");
  const [mode, setMode] = React.useState("abs");
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "repeat(4,1fr)",
      gap: "var(--chrome-gutter)",
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(MetricStat, {
    label: "Samples",
    value: "48,210",
    provenance: "measured",
    footnote: "perf record \xB7 997 Hz"
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "Wall clock",
    value: "412.7",
    unit: "ms",
    delta: 12.4,
    lowerIsBetter: true,
    provenance: "measured",
    onClick: () => {}
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "IPC (hot loop)",
    value: "0.44",
    provenance: "measured",
    footnote: "11.9% L1-d miss"
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "Vectorization",
    value: "missed",
    provenance: "inferred",
    footnote: "bound not loop-invariant"
  })), /*#__PURE__*/React.createElement(Panel, {
    title: "Flamegraph \xB7 inlining-aware",
    icon: "flame",
    padding: false,
    style: {
      flex: "none"
    },
    actions: /*#__PURE__*/React.createElement(SegmentedControl, {
      size: "sm",
      value: mode,
      onChange: setMode,
      options: [{
        value: "abs",
        label: "Absolute"
      }, {
        value: "diff",
        label: "Differential"
      }]
    }),
    footer: /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "hatched frames were inlined away \xB7 click a frame to jump the source pane")
  }, /*#__PURE__*/React.createElement(Flamegraph, {
    frames: L.flame,
    totalSamples: 48210,
    selectedId: fr,
    onSelect: x => setFr(x.id),
    rowHeight: 22
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Hot symbols",
    icon: "list-ordered",
    padding: false,
    style: {
      flex: 1,
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement(DataTable, {
    rows: L.hotSymbols,
    selectedIndex: 1,
    onSelect: () => {},
    columns: [{
      key: "sym",
      label: "Symbol",
      maxWidth: 300
    }, {
      key: "self",
      label: "Self",
      align: "right",
      width: 70
    }, {
      key: "total",
      label: "Total",
      align: "right",
      width: 70
    }, {
      key: "samples",
      label: "Samples",
      align: "right",
      width: 84,
      muted: true
    }, {
      key: "ipc",
      label: "IPC",
      align: "right",
      width: 60
    }, {
      key: "miss",
      label: "L1-d miss",
      align: "right",
      width: 84
    }]
  })), /*#__PURE__*/React.createElement(Panel, {
    title: "src/parser.rs \xB7 attributed",
    icon: "file-code",
    padding: false,
    scroll: true,
    style: {
      width: 380,
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(CodeBlock, {
    startLine: L.source.start,
    highlightLines: [142],
    annotations: {
      142: "27.8% of samples"
    },
    lines: L.source.lines
  })))), /*#__PURE__*/React.createElement(Inspector, {
    findings: L.findings.perf,
    selectedId: sel,
    onSelect: setSel,
    evidence: {},
    gates: []
  }));
}
Object.assign(window, {
  PerfLab
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/PerfLab.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/ProfileLab.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function ProfileLab() {
  const L = window.LODE;
  const [cfg, setCfg] = React.useState("z-fat");
  const [confirm, setConfirm] = React.useState(false);
  const cur = L.configs.find(c => c.id === cfg) || L.configs[0];
  const points = L.configs.map(c => ({
    id: c.id,
    x: c.rtNum,
    y: c.sizeKb,
    size: c.buildN,
    frontier: c.frontier,
    failed: c.gate === "fail"
  }));
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      position: "relative"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)"
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Profile Lab \xB7 36 configurations swept",
    icon: "scatter-chart",
    padding: false,
    style: {
      flex: "none"
    },
    actions: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("span", {
      style: {
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, "point size = build time"), /*#__PURE__*/React.createElement(IconButton, {
      icon: "rotate-cw",
      label: "Re-run sweep",
      size: "sm"
    }))
  }, /*#__PURE__*/React.createElement(ParetoScatter, {
    height: 252,
    points: points,
    selectedId: cfg,
    onSelect: p => setCfg(p.id)
  })), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: 1,
      minHeight: 0
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Configurations",
    icon: "sliders-horizontal",
    padding: false,
    style: {
      flex: 1,
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement(DataTable, {
    rows: L.configs,
    selectedIndex: L.configs.findIndex(c => c.id === cfg),
    onSelect: r => setCfg(r.id),
    columns: [{
      key: "flags",
      label: "Flags",
      maxWidth: 300
    }, {
      key: "size",
      label: "Size",
      align: "right",
      width: 82
    }, {
      key: "delta",
      label: "Δ size",
      align: "right",
      width: 82,
      render: r => /*#__PURE__*/React.createElement(DeltaValue, {
        value: r.delta,
        size: "sm",
        showIcon: false
      })
    }, {
      key: "rt",
      label: "Runtime",
      align: "right",
      width: 84
    }, {
      key: "build",
      label: "Build",
      align: "right",
      width: 70,
      muted: true
    }, {
      key: "gate",
      label: "Gates",
      width: 108,
      render: r => r.gate === "pass" ? /*#__PURE__*/React.createElement(Badge, {
        tone: "pass"
      }, "pass") : /*#__PURE__*/React.createElement(Badge, {
        tone: "fail"
      }, r.failedGate)
    }]
  })), /*#__PURE__*/React.createElement(Panel, {
    title: "Selected configuration",
    icon: "check-check",
    style: {
      width: 340,
      flex: "none"
    },
    footer: /*#__PURE__*/React.createElement("div", {
      style: {
        display: "flex",
        gap: 8,
        justifyContent: "flex-end"
      }
    }, /*#__PURE__*/React.createElement(Button, {
      size: "sm",
      variant: "secondary",
      icon: "clipboard-copy"
    }, "Copy profile"), /*#__PURE__*/React.createElement(Button, {
      size: "sm",
      variant: "primary",
      icon: "git-commit-horizontal",
      onClick: () => setConfirm(true)
    }, "Apply to Cargo.toml"))
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-code)",
      color: "var(--text-primary)"
    }
  }, cur.flags), /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "1fr 1fr",
      gap: 8
    }
  }, /*#__PURE__*/React.createElement(MetricStat, {
    label: "Size",
    value: cur.size,
    unit: "B",
    delta: cur.delta,
    provenance: "measured"
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "Runtime",
    value: cur.rt.replace(" ms", ""),
    unit: "ms",
    provenance: "measured",
    footnote: "hyperfine \xB7 10 runs"
  })), /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    style: {
      font: "var(--type-caps)",
      letterSpacing: "var(--tracking-caps)",
      textTransform: "uppercase",
      color: "var(--text-muted)",
      marginBottom: 6
    }
  }, "Gates"), L.gates.map(g => /*#__PURE__*/React.createElement(GateResult, _extends({
    key: g.gate
  }, g)))))))), /*#__PURE__*/React.createElement(Dialog, {
    open: confirm,
    width: 440,
    tone: "warn",
    title: "Apply configuration to Cargo.toml?",
    description: "Trust tier Propose does not apply changes. Raising to Tune lets Binmap write build configuration after tests pass and benchmarks confirm.",
    onClose: () => setConfirm(false),
    footer: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(Button, {
      variant: "ghost",
      onClick: () => setConfirm(false)
    }, "Cancel"), /*#__PURE__*/React.createElement(Button, {
      variant: "primary",
      icon: "arrow-up-right",
      onClick: () => setConfirm(false)
    }, "Raise to Tune and apply"))
  }, /*#__PURE__*/React.createElement(TrustTierControl, {
    variant: "list",
    value: "propose"
  })));
}
Object.assign(window, {
  ProfileLab
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/ProfileLab.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/ProjectView.jsx
try { (() => {
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function ProjectView({
  onRun
}) {
  const L = window.LODE;
  const [ai, setAi] = React.useState(true);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)",
      overflow: "auto"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "flex",
      gap: "var(--chrome-gutter)",
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(Panel, {
    title: "Project",
    icon: "folder-open",
    style: {
      flex: 1
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 8,
      font: "var(--type-code)",
      fontSize: "var(--fs-12)"
    }
  }, [["path", L.project.path], ["commit", L.project.commit + " (dirty)"], ["toolchain", L.project.toolchain], ["target", "x86_64-unknown-linux-gnu"], ["profile", "release · opt-level=3 · lto=off"], ["bench", "cargo bench --bench parse"]].map(([k, v]) => /*#__PURE__*/React.createElement("div", {
    key: k,
    style: {
      display: "flex",
      gap: 12
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      width: 76,
      color: "var(--text-muted)"
    }
  }, k), /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--text-primary)"
    }
  }, v))))), /*#__PURE__*/React.createElement(Panel, {
    title: "Run an analysis",
    icon: "play",
    style: {
      width: 400,
      flex: "none"
    },
    footer: /*#__PURE__*/React.createElement("div", {
      style: {
        display: "flex",
        alignItems: "center",
        gap: 8
      }
    }, /*#__PURE__*/React.createElement(Switch, {
      checked: ai,
      onChange: setAi,
      label: "Model layer",
      size: "sm"
    }), /*#__PURE__*/React.createElement("span", {
      style: {
        marginLeft: "auto",
        font: "var(--type-code)",
        fontSize: "var(--fs-11)",
        color: "var(--text-muted)"
      }
    }, ai ? "local/qwen3-coder-next" : "--no-ai · deterministic only"))
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gap: 8
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    icon: "hard-drive",
    fullWidth: true,
    onClick: () => onRun("size")
  }, "Attribute binary size"), /*#__PURE__*/React.createElement(Button, {
    variant: "secondary",
    icon: "sliders-horizontal",
    fullWidth: true,
    onClick: () => onRun("tune")
  }, "Sweep build configurations"), /*#__PURE__*/React.createElement(Button, {
    variant: "secondary",
    icon: "bug",
    fullWidth: true,
    onClick: () => onRun("crash")
  }, "Analyze core dump\u2026"), /*#__PURE__*/React.createElement(Button, {
    variant: "secondary",
    icon: "flame",
    fullWidth: true,
    onClick: () => onRun("perf")
  }, "Profile the benchmark")))), /*#__PURE__*/React.createElement(Panel, {
    title: "Run history",
    icon: "history",
    padding: false,
    style: {
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(DataTable, {
    rows: L.history,
    onSelect: () => onRun("size"),
    columns: [{
      key: "when",
      label: "When",
      width: 120,
      muted: true
    }, {
      key: "cmd",
      label: "Command",
      width: 240
    }, {
      key: "result",
      label: "Result",
      maxWidth: 320
    }, {
      key: "status",
      label: "",
      width: 90,
      render: r => /*#__PURE__*/React.createElement(Badge, {
        tone: r.status
      }, r.status === "pass" ? "complete" : r.status === "warn" ? "regressed" : "crash")
    }]
  })), /*#__PURE__*/React.createElement(Panel, {
    padding: false,
    style: {
      flex: 1,
      minHeight: 240
    }
  }, /*#__PURE__*/React.createElement(EmptyState, {
    icon: "compass",
    title: "Pick an analysis to begin",
    teach: "Binmap reads your source and your compiled binary at the same time. Every number it shows you was measured by a tool you can name, and every explanation cites the measurement it came from.",
    actions: /*#__PURE__*/React.createElement(Button, {
      variant: "primary",
      icon: "hard-drive",
      onClick: () => onRun("size")
    }, "Attribute binary size"),
    footnote: "binmap size --json"
  })));
}
Object.assign(window, {
  ProjectView
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/ProjectView.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/SizeExplorer.jsx
try { (() => {
const {
  Button,
  IconButton,
  Icon,
  Input,
  Select,
  Checkbox,
  Switch,
  SegmentedControl,
  Tabs,
  Panel,
  Dialog,
  Tooltip,
  Badge,
  Tag,
  DataTable,
  MetricStat,
  DeltaValue,
  ProgressBar,
  CodeBlock,
  ProvenanceBadge,
  ConfidenceBadge,
  EvidenceItem,
  FindingCard,
  ProposalBlock,
  GateResult,
  TrustTierControl,
  TranscriptStep,
  StackFrameList,
  Treemap,
  ParetoScatter,
  Flamegraph,
  TitleBar,
  NavRail,
  StatusBar,
  CommandPalette,
  EmptyState
} = window.SilverKeyLodestoneDesignSystem_2b7558;
function SizeExplorer() {
  const L = window.LODE;
  const [group, setGroup] = React.useState("crate");
  const [region, setRegion] = React.useState("fmt");
  const [row, setRow] = React.useState(1);
  const [sel, setSel] = React.useState("f1");
  const [diff, setDiff] = React.useState(false);
  return /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      flex: 1,
      minWidth: 0,
      display: "flex",
      flexDirection: "column",
      gap: "var(--chrome-gutter)",
      padding: "var(--chrome-gutter)"
    }
  }, /*#__PURE__*/React.createElement("div", {
    style: {
      display: "grid",
      gridTemplateColumns: "repeat(4,1fr)",
      gap: "var(--chrome-gutter)",
      flex: "none"
    }
  }, /*#__PURE__*/React.createElement(MetricStat, {
    label: "Binary size",
    value: "1,214,336",
    unit: "bytes",
    delta: -38.2,
    provenance: "measured",
    onClick: () => {},
    footnote: "bloaty 1.1.0 \xB7 stripped"
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: ".text",
    value: "731,904",
    unit: "bytes",
    delta: -31.0,
    provenance: "measured",
    onClick: () => {}
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "Findings",
    value: "4",
    provenance: "derived",
    footnote: "1 model claim, 3 measured"
  }), /*#__PURE__*/React.createElement(MetricStat, {
    label: "Est. further saving",
    value: "255",
    unit: "KB",
    provenance: "inferred",
    footnote: "two proposals, unverified"
  })), /*#__PURE__*/React.createElement(Panel, {
    title: diff ? "Size Explorer · diff against HEAD~1" : "Size Explorer",
    icon: "hard-drive",
    padding: false,
    style: {
      flex: 1,
      minHeight: 300
    },
    actions: /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(SegmentedControl, {
      size: "sm",
      value: group,
      onChange: setGroup,
      options: [{
        value: "crate",
        label: "Crate"
      }, {
        value: "generic",
        label: "Generic"
      }, {
        value: "section",
        label: "Section"
      }]
    }), /*#__PURE__*/React.createElement(IconButton, {
      icon: "git-compare",
      label: "Diff mode",
      size: "sm",
      active: diff,
      onClick: () => setDiff(!diff)
    }), /*#__PURE__*/React.createElement(IconButton, {
      icon: "maximize-2",
      label: "Expand",
      size: "sm"
    })),
    bodyStyle: {
      display: "flex",
      minHeight: 0
    },
    footer: /*#__PURE__*/React.createElement("div", {
      style: {
        display: "flex",
        gap: 6,
        flexWrap: "wrap"
      }
    }, L.treemap.map(t => /*#__PURE__*/React.createElement(Tag, {
      key: t.id,
      color: t.color
    }, t.label)))
  }, /*#__PURE__*/React.createElement(Treemap, {
    data: L.treemap,
    height: "100%",
    selectedId: region,
    onSelect: n => setRegion(n.id)
  })), /*#__PURE__*/React.createElement(Panel, {
    title: "Symbols",
    icon: "list",
    padding: false,
    style: {
      flex: "none",
      height: 214,
      minHeight: 140
    },
    actions: /*#__PURE__*/React.createElement("div", {
      style: {
        width: 220
      }
    }, /*#__PURE__*/React.createElement(Input, {
      size: "sm",
      icon: "search",
      mono: true,
      placeholder: "Filter symbols\u2026"
    }))
  }, /*#__PURE__*/React.createElement(DataTable, {
    selectedIndex: row,
    onSelect: (r, i) => setRow(i),
    zebra: true,
    rows: L.symbols,
    columns: [{
      key: "sym",
      label: "Symbol",
      maxWidth: 330
    }, {
      key: "crate",
      label: "Crate",
      width: 92
    }, {
      key: "bytes",
      label: "Bytes",
      align: "right",
      width: 78
    }, {
      key: "n",
      label: "Inst.",
      align: "right",
      width: 54
    }, {
      key: "share",
      label: "Share",
      align: "right",
      width: 62,
      muted: true
    }, {
      key: "src",
      label: "Source",
      maxWidth: 190,
      muted: true
    }]
  }))), /*#__PURE__*/React.createElement(Inspector, {
    findings: L.findings.size,
    selectedId: sel,
    onSelect: setSel,
    evidence: L.evidence,
    gates: [],
    proposal: sel === "f1" ? {
      summary: "Collapse 11 instantiations behind &dyn Error",
      tier: "Propose",
      blocked: "Tier 3 required to patch source",
      diff: ["@@ -88,7 +88,7 @@ impl Config {", "-    pub fn load<E: Error>(s: &str) -> Result<Cfg, E> {", "+    pub fn load(s: &str) -> Result<Cfg, Box<dyn Error>> {", "         let raw = serde_json::from_str(s)?;", "         Ok(Cfg::from(raw))"]
    } : sel === "f3" ? {
      summary: "Set panic = \"abort\" in [profile.release]",
      tier: "Propose",
      diff: ["@@ -12,3 +12,4 @@ [profile.release]", " lto = \"fat\"", " codegen-units = 1", "+panic = \"abort\""]
    } : null
  }));
}
Object.assign(window, {
  SizeExplorer
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/SizeExplorer.jsx", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/data.js
try { (() => {
// Fixture data for the Binmap desktop UI kit. Values are illustrative but
// internally consistent: the treemap, tables and findings all describe one build
// of "my-crate" @ a3f9c21.
window.LODE = {
  project: {
    name: "my-crate",
    commit: "a3f9c21",
    dirty: true,
    toolchain: "1.82.0 (x86_64-unknown-linux-gnu)",
    path: "~/src/my-crate"
  },
  treemap: [{
    id: "mine",
    label: "my_crate",
    value: 412000,
    display: "412 KB",
    color: "var(--cat-yours)"
  }, {
    id: "fmt",
    label: "core::fmt",
    value: 214880,
    display: "214 KB",
    color: "var(--cat-fmt)"
  }, {
    id: "serde",
    label: "serde_json",
    value: 186400,
    display: "186 KB",
    color: "var(--cat-deps)"
  }, {
    id: "unwind",
    label: ".eh_frame",
    value: 148000,
    display: "148 KB",
    color: "var(--cat-unwind)"
  }, {
    id: "panic",
    label: "panic strings",
    value: 96200,
    display: "96 KB",
    color: "var(--cat-panic)"
  }, {
    id: "drop",
    label: "Drop glue",
    value: 64800,
    display: "64 KB",
    color: "var(--cat-drop)"
  }, {
    id: "vtable",
    label: "vtables",
    value: 41200,
    display: "41 KB",
    color: "var(--cat-vtable)"
  }, {
    id: "static",
    label: "static data",
    value: 50856,
    display: "50 KB",
    color: "var(--cat-static)"
  }],
  symbols: [{
    sym: "core::fmt::Formatter::pad_integral",
    crate: "core",
    bytes: "40,112",
    n: "1",
    share: "3.3%",
    src: "core/src/fmt/mod.rs:1402"
  }, {
    sym: "serde_json::de::from_str::<Config>",
    crate: "serde_json",
    bytes: "31,904",
    n: "12",
    share: "2.6%",
    src: "src/config.rs:88"
  }, {
    sym: "my_crate::parse::Parser::run",
    crate: "my_crate",
    bytes: "22,448",
    n: "1",
    share: "1.9%",
    src: "src/parser.rs:61"
  }, {
    sym: "<alloc::vec::Vec<T> as Drop>::drop",
    crate: "alloc",
    bytes: "18,320",
    n: "31",
    share: "1.5%",
    src: "alloc/src/vec/mod.rs:3062"
  }, {
    sym: "my_crate::config::Config::validate",
    crate: "my_crate",
    bytes: "14,776",
    n: "1",
    share: "1.2%",
    src: "src/config.rs:210"
  }, {
    sym: "core::panicking::panic_bounds_check",
    crate: "core",
    bytes: "9,984",
    n: "1",
    share: "0.8%",
    src: "core/src/panicking.rs:206"
  }, {
    sym: "<&T as Display>::fmt",
    crate: "core",
    bytes: "8,640",
    n: "18",
    share: "0.7%",
    src: "core/src/fmt/mod.rs:2451"
  }, {
    sym: "my_crate::io::read_all",
    crate: "my_crate",
    bytes: "7,208",
    n: "1",
    share: "0.6%",
    src: "src/io.rs:33"
  }],
  configs: [{
    id: "z-fat",
    flags: "opt-level=z, lto=fat, cgu=1, panic=abort",
    size: "1,214,336",
    sizeKb: 1186,
    delta: -38.2,
    rt: "412.7 ms",
    rtNum: 412,
    build: "2m 04s",
    buildN: 0.9,
    gate: "pass",
    frontier: true
  }, {
    id: "s-thin",
    flags: "opt-level=s, lto=thin, cgu=16",
    size: "1,489,120",
    sizeKb: 1454,
    delta: -24.4,
    rt: "388.1 ms",
    rtNum: 388,
    build: "1m 41s",
    buildN: 0.55,
    gate: "pass",
    frontier: true
  }, {
    id: "3-off",
    flags: "opt-level=3, lto=off, cgu=16 (default)",
    size: "1,966,208",
    sizeKb: 1920,
    delta: 0,
    rt: "381.4 ms",
    rtNum: 381,
    build: "1m 12s",
    buildN: 0.3,
    gate: "pass",
    frontier: true
  }, {
    id: "2-thin",
    flags: "opt-level=2, lto=thin, cgu=8",
    size: "1,845,248",
    sizeKb: 1802,
    delta: -6.2,
    rt: "396.0 ms",
    rtNum: 396,
    build: "1m 28s",
    buildN: 0.4,
    gate: "pass"
  }, {
    id: "z-thin",
    flags: "opt-level=z, lto=thin, cgu=1",
    size: "1,269,760",
    sizeKb: 1240,
    delta: -35.4,
    rt: "441.2 ms",
    rtNum: 441,
    build: "1m 52s",
    buildN: 0.5,
    gate: "pass"
  }, {
    id: "s-fat",
    flags: "opt-level=s, lto=fat, cgu=1",
    size: "1,421,312",
    sizeKb: 1388,
    delta: -27.7,
    rt: "402.4 ms",
    rtNum: 402,
    build: "2m 18s",
    buildN: 0.8,
    gate: "pass"
  }, {
    id: "z-abort-std",
    flags: "opt-level=z, build-std, panic_immediate_abort",
    size: "1,130,496",
    sizeKb: 1104,
    delta: -42.5,
    rt: "398.8 ms",
    rtNum: 398,
    build: "6m 02s",
    buildN: 0.85,
    gate: "fail",
    failedGate: "TestsPass"
  }, {
    id: "3-fat",
    flags: "opt-level=3, lto=fat, cgu=1",
    size: "2,059,264",
    sizeKb: 2011,
    delta: 4.7,
    rt: "376.1 ms",
    rtNum: 376,
    build: "3m 40s",
    buildN: 1,
    gate: "fail",
    failedGate: "SizeNotWorse"
  }],
  frames: [{
    symbol: "my_crate::parse::Parser::index",
    location: "src/parser.rs:142",
    addr: "0x00401f2a",
    mappingMethod: "DwarfLineTable"
  }, {
    symbol: "core::slice::index::panic_bounds_check",
    location: "core/src/slice/index.rs:36",
    inlineDepth: 1,
    provenance: "derived",
    mappingMethod: "InlineFrame { depth: 1 }"
  }, {
    symbol: "core::panicking::panic_fmt",
    location: "core/src/panicking.rs:72",
    addr: "0x00408c10",
    mappingMethod: "SymbolTable"
  }, {
    symbol: "my_crate::parse::Parser::run",
    location: "src/parser.rs:61",
    addr: "0x00401a04",
    mappingMethod: "DwarfLineTable"
  }, {
    symbol: "my_crate::main",
    location: "src/main.rs:24",
    addr: "0x004013f0",
    mappingMethod: "DwarfLineTable"
  }],
  source: {
    start: 136,
    highlight: [142],
    annotations: {
      141: "len = 4096",
      142: "idx = 4096  \u25c8 recovered from r13"
    },
    lines: [[["cm", "    /// Returns the byte at `idx`. Callers are expected to bounds-check."]], [["kw", "    pub fn "], ["fn", "index"], ["punct", "(&"], ["kw", "self"], ["punct", ", "], ["plain", "idx"], ["punct", ": "], ["ty", "usize"], ["punct", ") -> "], ["ty", "u8"], ["punct", " {"]], [["plain", "        let buf = &"], ["kw", "self"], ["plain", ".buffer;"]], [["cm", "        // fast path: caller already validated the window"]], [["plain", "        "], ["kw", "debug_assert!"], ["punct", "("], ["plain", "idx <= buf.len()"], ["punct", ");"]], [["plain", "        "], ["punct", "buf["], ["plain", "idx"], ["punct", "]"]], [["punct", "    }"]], [""], [["kw", "    fn "], ["fn", "window"], ["punct", "(&"], ["kw", "self"], ["punct", ") -> "], ["punct", "&["], ["ty", "u8"], ["punct", "] {"]]]
  },
  disasm: {
    start: 1,
    highlight: [4],
    lines: [[["addr", "0x00401f1a"], ["plain", "  "], ["mnem", "mov"], ["plain", "    "], ["reg", "rax"], ["punct", ", "], ["punct", "qword ptr ["], ["reg", "rdi"], ["punct", " + "], ["num", "0x10"], ["punct", "]"]], [["addr", "0x00401f1e"], ["plain", "  "], ["mnem", "mov"], ["plain", "    "], ["reg", "r13"], ["punct", ", "], ["reg", "rsi"], ["plain", "                 "], ["cm", "; idx"]], [["addr", "0x00401f21"], ["plain", "  "], ["mnem", "cmp"], ["plain", "    "], ["reg", "r13"], ["punct", ", "], ["reg", "rax"]], [["addr", "0x00401f24"], ["plain", "  "], ["mnem", "jae"], ["plain", "    "], ["fn", "panic_bounds_check"], ["plain", "   "], ["cm", "; taken"]], [["addr", "0x00401f2a"], ["plain", "  "], ["mnem", "movzx"], ["plain", "  "], ["reg", "eax"], ["punct", ", "], ["punct", "byte ptr ["], ["reg", "rdx"], ["punct", " + "], ["reg", "r13"], ["punct", "]"]], [["addr", "0x00401f2f"], ["plain", "  "], ["mnem", "ret"]]]
  },
  flame: [{
    id: "root",
    label: "my_crate::main",
    x: 0,
    w: 100,
    depth: 0,
    samples: 48210,
    color: "var(--cat-yours)"
  }, {
    id: "parse",
    label: "my_crate::parse::Parser::run",
    x: 2,
    w: 64,
    depth: 1,
    samples: 29800,
    color: "var(--cat-yours)"
  }, {
    id: "serde",
    label: "serde_json::de::from_str",
    x: 68,
    w: 30,
    depth: 1,
    samples: 14100,
    color: "var(--cat-deps)"
  }, {
    id: "idx",
    label: "slice::index (inlined)",
    x: 6,
    w: 34,
    depth: 2,
    samples: 13400,
    inlined: true,
    color: "var(--cat-yours)"
  }, {
    id: "fmtw",
    label: "core::fmt::write",
    x: 42,
    w: 22,
    depth: 2,
    samples: 8600,
    color: "var(--cat-fmt)"
  }, {
    id: "memcpy",
    label: "__memcpy_avx_unaligned",
    x: 70,
    w: 24,
    depth: 2,
    samples: 9100,
    color: "var(--cat-deps)"
  }, {
    id: "pad",
    label: "pad_integral (inlined)",
    x: 44,
    w: 14,
    depth: 3,
    samples: 5200,
    inlined: true,
    color: "var(--cat-fmt)"
  }, {
    id: "utf8",
    label: "core::str::validate_utf8",
    x: 72,
    w: 14,
    depth: 3,
    samples: 4400,
    color: "var(--cat-deps)"
  }],
  hotSymbols: [{
    sym: "my_crate::parse::Parser::run",
    self: "18.4%",
    total: "61.8%",
    samples: "8,872",
    ipc: "0.71",
    miss: "4.1%"
  }, {
    sym: "slice::index (inlined into run)",
    self: "27.8%",
    total: "27.8%",
    samples: "13,400",
    ipc: "0.44",
    miss: "11.9%"
  }, {
    sym: "serde_json::de::from_str",
    self: "10.4%",
    total: "29.2%",
    samples: "5,012",
    ipc: "0.88",
    miss: "2.2%"
  }, {
    sym: "__memcpy_avx_unaligned",
    self: "18.9%",
    total: "18.9%",
    samples: "9,100",
    ipc: "1.92",
    miss: "0.9%"
  }, {
    sym: "core::fmt::write",
    self: "7.1%",
    total: "17.8%",
    samples: "3,420",
    ipc: "0.63",
    miss: "3.4%"
  }],
  findings: {
    size: [{
      id: "f1",
      kind: "MonomorphizationBloat",
      title: "serde_json::from_str monomorphized 12 times",
      detail: "Eleven instantiations differ only in their error type. Collapsing them behind &dyn Error is likely to be behaviour-preserving.",
      location: "src/config.rs:88",
      impact: "\u2212214 KB (10.9%)",
      confidence: "probable",
      provenance: "inferred",
      evidenceCount: 6
    }, {
      id: "f2",
      kind: "SizeDriver",
      title: "core::fmt machinery totals 214,880 bytes",
      detail: "Formatting code reachable from 41 call sites, 18 of them in panic messages.",
      location: ".text \u00b7 core::fmt",
      impact: "17.6% of .text",
      confidence: "certain",
      provenance: "measured",
      evidenceCount: 2
    }, {
      id: "f3",
      kind: "ConfigOpportunity",
      title: "panic = abort removes 148 KB of unwinding tables",
      detail: "Measured across the sweep. Changes semantics: no unwinding, no catch_unwind.",
      location: "Cargo.toml \u00b7 [profile.release]",
      impact: "\u2212148 KB (7.5%)",
      confidence: "certain",
      provenance: "measured",
      evidenceCount: 3
    }, {
      id: "f4",
      kind: "DeadCode",
      title: "41 KB of vtables retained for unused trait objects",
      detail: "Three traits are object-safe and boxed once at startup, but never dispatched.",
      location: ".data.rel.ro",
      impact: "\u221241 KB (2.1%)",
      confidence: "high",
      provenance: "derived",
      evidenceCount: 4
    }],
    crash: [{
      id: "c1",
      kind: "CrashCause",
      title: "Index out of bounds: idx == len == 4096",
      detail: "Parser::index is called with the window length rather than the last valid offset. The debug_assert is compiled out in release, so the bound is never checked.",
      location: "src/parser.rs:142 \u00b7 0x00401f2a",
      impact: "IndexOutOfBounds",
      confidence: "probable",
      provenance: "inferred",
      evidenceCount: 5
    }],
    perf: [{
      id: "p1",
      kind: "PerfHotspot",
      title: "Bounds check in the parse loop blocks vectorization",
      detail: "27.8% of samples land in an inlined slice index whose bound is not loop-invariant. IPC 0.44 with 11.9% L1 misses.",
      location: "src/parser.rs:142",
      impact: "27.8% of samples",
      confidence: "probable",
      provenance: "inferred",
      evidenceCount: 4
    }, {
      id: "p2",
      kind: "Regression",
      title: "parse benchmark regressed 12.4% since 9f2c118",
      detail: "Regression is confined to the inlined index path; the surrounding function is unchanged.",
      location: "benches/parse.rs",
      impact: "+12.4%",
      confidence: "certain",
      provenance: "measured",
      evidenceCount: 3
    }]
  },
  evidence: {
    f1: [{
      kind: "measured",
      summary: "12 instantiations share generic origin from_str<T>",
      tool: "binmap-core 0.4.1",
      args: "group_monomorphizations --min-bytes 4096",
      digest: "7c41e0\u20268a2f",
      payload: "generic_origin: serde_json::de::from_str<T>\n  instantiations: 12\n  aggregate:      214,880 bytes\n  distinct type params: Config, Manifest, Lockfile, ...\n  differing only in E: 11"
    }, {
      kind: "measured",
      summary: "core::fmt machinery totals 214,880 bytes",
      tool: "bloaty 1.1.0",
      args: "--domain=vm -d symbols",
      digest: "a3f9c2\u20268e4d",
      payload: "    FILE SIZE        VM SIZE\n --------------  --------------\n  214Ki  17.6%    198Ki  18.1%   core::fmt\n  148Ki  12.2%      0     0.0%   .eh_frame\n   96Ki   7.9%     96Ki   8.8%   [panic strings]"
    }, {
      kind: "derived",
      summary: "Error types are the only differing type parameter",
      tool: "rule: mono_collapse_candidate",
      args: "",
      payload: "matched 11 of 12 instantiations\n  shared:    fn(&str) -> Result<_, _>\n  differing: E only"
    }, {
      kind: "inferred",
      summary: "Collapse behind &dyn Error is behaviour-preserving here",
      tool: "local/qwen3-coder-next",
      args: "cites 3 evidence items",
      payload: "The eleven instantiations are only reached from Config::load and its\ncallers, all of which discard the concrete error type before returning.\nBoxing the error therefore changes no observable behaviour."
    }],
    c1: [{
      kind: "measured",
      summary: "Faulting address 0x00401f2a maps to src/parser.rs:142",
      tool: "addr2line 0.24",
      args: "-e target/release/my-crate -f -i 0x401f2a",
      digest: "b1d7f4\u2026003a",
      payload: "my_crate::parse::Parser::index\n  src/parser.rs:142\n(inlined by) my_crate::parse::Parser::run\n  src/parser.rs:61"
    }, {
      kind: "measured",
      summary: "SIGSEGV \u2192 panic_bounds_check reached from index",
      tool: "lldb 18.1",
      args: "-c core.1234 --batch -o 'bt all'",
      payload: "* thread #1, name = 'my-crate', stop reason = signal SIGABRT\n  frame #0: core::panicking::panic_bounds_check\n  frame #1: my_crate::parse::Parser::index at parser.rs:142"
    }, {
      kind: "measured",
      summary: "buf.len() == 4096 read from DWARF location list",
      tool: "gimli 0.31",
      args: "dwarf_query --var buf --pc 0x401f2a",
      payload: "DW_AT_location: DW_OP_fbreg -72\n  buffer.len = 4096 (u64 @ rbp-0x48)"
    }, {
      kind: "derived",
      summary: "idx recovered from r13 by backward slice",
      tool: "rule: register_recovery",
      args: "",
      payload: "0x00401f1e  mov r13, rsi   ; idx, argument 2\nno intervening write to r13\n  => idx = 4096 at 0x00401f2a"
    }, {
      kind: "inferred",
      summary: "Caller passes window length instead of last offset",
      tool: "local/qwen3-coder-next",
      args: "cites 4 evidence items",
      payload: "Parser::run computes `end = start + len` and calls index(end).\nThe debug_assert at parser.rs:140 would have caught this in a debug\nbuild; it is compiled out at opt-level=3."
    }]
  },
  gates: [{
    gate: "Builds",
    status: "pass",
    measurement: "2m 04s"
  }, {
    gate: "TestsPass",
    status: "pass",
    measurement: "184 passed"
  }, {
    gate: "NoNewWarnings",
    status: "pass",
    measurement: "0 new"
  }, {
    gate: "BenchmarkNotWorse { significance: 0.05 }",
    status: "inconclusive",
    measurement: "+1.1% n.s.",
    note: "inside noise floor (0.9%)"
  }, {
    gate: "SizeNotWorse",
    status: "pass",
    measurement: "\u2212752 KB"
  }, {
    gate: "MiriClean",
    status: "skipped",
    note: "no unsafe touched"
  }],
  transcript: [{
    step: 1,
    tool: "project_info",
    cost: "0.9k tok \u00b7 0.2s",
    hypothesis: "The binary's size is dominated by generic instantiations rather than by static data.",
    refutedBy: "A .rodata section larger than .text",
    args: '{ "root": "~/src/my-crate" }',
    result: "workspace: 1 crate, 1 bin target\nprofile: release (opt-level=3, lto=off)\n.text 1,182,208  .rodata 412,904  .eh_frame 148,000"
  }, {
    step: 2,
    tool: "section_breakdown",
    cost: "1.4k tok \u00b7 0.4s",
    hypothesis: "Unwinding tables and formatting machinery together exceed 300 KB.",
    refutedBy: ".eh_frame below 64 KB",
    args: '{ "binary": "target/release/my-crate" }',
    result: ".eh_frame  148,000\ncore::fmt  214,880\npanic str   96,200"
  }, {
    step: 3,
    tool: "group_monomorphizations",
    cost: "4.2k tok \u00b7 1.1s",
    hypothesis: "Most of core::fmt's cost comes from Display impls on error types.",
    refutedBy: "Fewer than 3 instantiations per error type",
    args: '{ "crate": "my-crate", "min_bytes": 4096 }',
    result: "11 groups \u00b7 214,880 bytes\ntop: serde_json::de::from_str<T>  12 \u00d7 17,906 avg"
  }, {
    step: 4,
    tool: "read_source_span",
    cost: "2.8k tok \u00b7 0.3s",
    hypothesis: "The twelve call sites discard the concrete error type, so boxing is free.",
    refutedBy: "Any caller matching on a concrete error variant",
    args: '{ "file": "src/config.rs", "lines": [80, 120] }',
    result: "12 call sites \u00b7 0 concrete matches on E"
  }],
  history: [{
    when: "today 14:02",
    cmd: "binmap tune",
    result: "36 configs \u00b7 best \u221238.2%",
    status: "pass"
  }, {
    when: "today 11:47",
    cmd: "binmap size --diff HEAD~1",
    result: "7 findings \u00b7 +12 KB",
    status: "warn"
  }, {
    when: "yesterday",
    cmd: "binmap analyze core.1234",
    result: "IndexOutOfBounds \u00b7 parser.rs:142",
    status: "fail"
  }, {
    when: "yesterday",
    cmd: "binmap profile -- ./bench",
    result: "48,210 samples",
    status: "pass"
  }]
};
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/data.js", error: String((e && e.message) || e) }); }

// ui_kits/binmap-desktop/model-catalog.js
try { (() => {
/* Model and agent catalog — checked against provider documentation on 6 September 2026.
   Binmap fetches each provider's live model list at runtime (Anthropic /v1/models,
   OpenAI /v1/models, and the OpenAI-compatible /v1/models on every other provider);
   this file is the offline fallback and the source of the display metadata.
   Every entry carries `released` — ModelPicker sorts newest-first from it. Retired
   models (kimi-k2.5, moonshot-v1, claude-3.x, gpt-4.x) are intentionally absent. */
window.LODE_MODELS = {
  providers: [{
    provider: "Anthropic",
    kind: "native",
    transport: "api.anthropic.com/v1",
    env: "ANTHROPIC_API_KEY",
    connected: true,
    keyFingerprint: "sk-ant-…q7Xa · added 12 Aug",
    status: "valid",
    hint: "Model IDs are pinned snapshots; from the 4.6 generation on they are dateless.",
    models: [{
      id: "claude-fable-5-1",
      name: "Claude Fable 5.1",
      released: "2026-09-01",
      context: "1M ctx",
      price: "$8 / $40 per M",
      tags: ["frontier", "tools"]
    }, {
      id: "claude-opus-5",
      name: "Claude Opus 5",
      released: "2026-07-24",
      context: "1M ctx",
      price: "$5 / $25 per M",
      tags: ["effort", "tools", "vision"]
    }, {
      id: "claude-fable-5",
      name: "Claude Fable 5",
      released: "2026-06-09",
      context: "1M ctx",
      price: "$8 / $40 per M",
      tags: ["tools"]
    }, {
      id: "claude-sonnet-5",
      name: "Claude Sonnet 5",
      released: "2026-06-09",
      context: "1M ctx",
      price: "$3 / $15 per M",
      tags: ["tools", "vision"]
    }, {
      id: "claude-opus-4-8",
      name: "Claude Opus 4.8",
      released: "2026-05-28",
      context: "1M ctx",
      price: "$5 / $25 per M",
      tags: ["tools"]
    }, {
      id: "claude-sonnet-4-6",
      name: "Claude Sonnet 4.6",
      released: "2026-02-17",
      context: "200K ctx",
      price: "$3 / $15 per M"
    }, {
      id: "claude-haiku-4-5",
      name: "Claude Haiku 4.5",
      released: "2025-10-15",
      context: "200K ctx",
      price: "$1 / $5 per M",
      tags: ["fast"]
    }, {
      id: "claude-mythos-5-1",
      name: "Claude Mythos 5.1",
      released: "2026-09-01",
      context: "1M ctx",
      available: false,
      tags: ["Project Glasswing"]
    }]
  }, {
    provider: "OpenAI",
    kind: "native",
    transport: "api.openai.com/v1",
    env: "OPENAI_API_KEY",
    connected: true,
    keyFingerprint: "sk-proj-…4Kd2 · added 3 Sep",
    status: "valid",
    hint: "gpt-5.6 is an alias for gpt-5.6-sol. Codex models are Responses-API only.",
    models: [{
      id: "gpt-6-astra",
      name: "GPT-6 Astra",
      released: "2026-08-20",
      context: "1.05M ctx",
      tags: ["frontier"]
    }, {
      id: "gpt-5.6-sol",
      name: "GPT-5.6 Sol",
      released: "2026-07-09",
      context: "1.05M ctx",
      price: "$5 / $30 per M",
      tags: ["tools"]
    }, {
      id: "gpt-5.6-terra",
      name: "GPT-5.6 Terra",
      released: "2026-07-09",
      context: "1.05M ctx",
      price: "$2 / $12 per M",
      tags: ["balanced"]
    }, {
      id: "gpt-5.6-luna",
      name: "GPT-5.6 Luna",
      released: "2026-07-09",
      context: "1.05M ctx",
      price: "$0.20 / $1.20 per M",
      tags: ["cheap"]
    }, {
      id: "gpt-5.5",
      name: "GPT-5.5",
      released: "2026-05-14",
      context: "400K ctx",
      price: "$5 / $30 per M"
    }, {
      id: "gpt-5.4",
      name: "GPT-5.4",
      released: "2026-03-05",
      context: "400K ctx",
      price: "$5 / $22.50 per M"
    }, {
      id: "gpt-5.3-codex",
      name: "GPT-5.3 Codex",
      released: "2026-02-05",
      context: "400K ctx",
      price: "$1.75 / $14 per M",
      tags: ["coding"]
    }, {
      id: "gpt-5-codex",
      name: "GPT-5 Codex",
      released: "2025-09-23",
      context: "400K ctx",
      price: "$1.25 / $10 per M",
      tags: ["coding"]
    }]
  }, {
    provider: "z.ai (GLM)",
    kind: "openai-compatible",
    transport: "api.z.ai/api/paas/v4",
    env: "ZAI_API_KEY",
    connected: true,
    keyFingerprint: "zai-…8Bc1 · added 1 Sep",
    status: "valid",
    hint: "Coding Plan keys route GLM-5.2 and GLM-5.1 requests to GLM-5.3, and GLM-4.7 to GLM-5.3-Flash.",
    models: [{
      id: "glm-5.3-flash",
      name: "GLM-5.3-Flash",
      released: "2026-08-26",
      context: "1.31M ctx",
      price: "$0.075 / $0.25 per M",
      tags: ["fast", "vision"]
    }, {
      id: "glm-5.3",
      name: "GLM-5.3",
      released: "2026-08-13",
      context: "1M ctx",
      price: "$1.40 / $4.40 per M",
      tags: ["coding", "tools"]
    }, {
      id: "glm-5.2",
      name: "GLM-5.2",
      released: "2026-06-16",
      context: "1M ctx",
      tags: ["routed → 5.3"]
    }, {
      id: "glm-5-turbo",
      name: "GLM-5-Turbo",
      released: "2026-05-20",
      context: "256K ctx",
      tags: ["Coding Plan"]
    }, {
      id: "glm-4.7",
      name: "GLM-4.7",
      released: "2026-01-28",
      context: "200K ctx",
      tags: ["routed → 5.3-Flash"]
    }]
  }, {
    provider: "Moonshot (Kimi)",
    kind: "openai-compatible",
    transport: "api.moonshot.ai/v1",
    env: "MOONSHOT_API_KEY",
    connected: false,
    hint: "OpenAI- and Anthropic-compatible surfaces. kimi-k2.5 and the moonshot-v1 series retired 31 Aug 2026.",
    models: [{
      id: "kimi-k3",
      name: "Kimi K3",
      released: "2026-07-16",
      context: "1M ctx",
      price: "$3 / $15 per M",
      tags: ["vision", "thinking"]
    }, {
      id: "kimi-k3-swarm-max",
      name: "Kimi K3 Swarm Max",
      released: "2026-07-16",
      context: "1M ctx",
      tags: ["multi-agent"]
    }, {
      id: "kimi-k2.7-code",
      name: "Kimi K2.7 Code",
      released: "2026-05-06",
      context: "256K ctx",
      price: "$0.95 / $4 per M",
      tags: ["coding"]
    }, {
      id: "kimi-k2.7-code-highspeed",
      name: "Kimi K2.7 Code Highspeed",
      released: "2026-05-06",
      context: "256K ctx",
      tags: ["low latency"]
    }, {
      id: "kimi-k2.6",
      name: "Kimi K2.6",
      released: "2026-03-11",
      context: "256K ctx",
      price: "$0.95 / $4 per M"
    }]
  }, {
    provider: "DeepSeek",
    kind: "openai-compatible",
    transport: "api.deepseek.com/v1",
    env: "DEEPSEEK_API_KEY",
    connected: true,
    keyFingerprint: "sk-…19Fa · added 28 Aug",
    status: "valid",
    hint: "Off-peak discounts apply automatically. Vision route is experimental.",
    models: [{
      id: "deepseek-v4-pro",
      name: "DeepSeek V4 Pro",
      released: "2026-04-24",
      context: "1M ctx",
      price: "$0.435 / $0.87 per M",
      tags: ["open weights"]
    }, {
      id: "deepseek-v4-flash",
      name: "DeepSeek V4 Flash",
      released: "2026-04-24",
      context: "1M ctx",
      price: "$0.14 / $0.28 per M",
      tags: ["cheap"]
    }, {
      id: "deepseek-v4-flash-vision-exp",
      name: "DeepSeek V4 Flash Vision",
      released: "2026-07-31",
      context: "1M ctx",
      tags: ["experimental", "vision"]
    }]
  }, {
    provider: "Local · llama.cpp",
    kind: "local",
    transport: "127.0.0.1:8080/v1 · OpenAI-compatible",
    env: "none",
    connected: true,
    status: "valid",
    keyFingerprint: "no key required",
    hint: "Default backend. Nothing leaves the machine; --no-ai disables the model layer entirely.",
    models: [{
      id: "qwen3-coder-next-q5",
      name: "Qwen3-Coder-Next (Q5_K_M)",
      released: "2026-06-02",
      context: "256K ctx",
      price: "local",
      tags: ["default"]
    }, {
      id: "glm-5.3-flash-q4",
      name: "GLM-5.3-Flash (Q4_K_M)",
      released: "2026-08-26",
      context: "128K ctx",
      price: "local"
    }, {
      id: "deepseek-v4-flash-q4",
      name: "DeepSeek V4 Flash (Q4_K_M)",
      released: "2026-04-24",
      context: "128K ctx",
      price: "local"
    }]
  }, {
    provider: "Custom · OpenAI-compatible",
    kind: "openai-compatible",
    transport: "user-supplied /v1",
    env: "BINMAP_CUSTOM_API_KEY",
    connected: false,
    custom: true,
    hint: "Any endpoint that implements /v1/chat/completions and /v1/models: vLLM, Ollama, LM Studio, OpenRouter, Together, Fireworks, Groq, Azure OpenAI, Bedrock gateways.",
    models: []
  }],
  acpAgents: [{
    name: "Claude Agent",
    vendor: "Anthropic · via Zed SDK adapter",
    command: "npx @zed-industries/claude-code-acp",
    state: "connected",
    version: "0.16.2",
    capabilities: ["fs.read", "fs.write", "terminal", "permissions", "session/update"],
    authNote: "Owns its own auth and billing. Run /login inside the agent — an Anthropic key set in Providers does not configure it."
  }, {
    name: "Codex CLI",
    vendor: "OpenAI · via zed-industries/codex-acp",
    command: "codex-acp --stdio",
    state: "needsLogin",
    version: "1.4.0",
    capabilities: ["fs.read", "fs.write", "terminal", "streaming"],
    authNote: "Accepts ChatGPT login, a Codex key, or an OpenAI key, and reads Codex-native config."
  }, {
    name: "Gemini CLI",
    vendor: "Google · native ACP",
    command: "gemini --experimental-acp",
    state: "installed",
    version: "0.9.4",
    capabilities: ["fs.read", "fs.write", "terminal"],
    authNote: "Original ACP launch partner; reference implementation."
  }, {
    name: "GitHub Copilot CLI",
    vendor: "GitHub · native ACP (public preview)",
    command: "copilot --acp",
    state: "installed",
    version: "0.6.1-preview",
    capabilities: ["fs.read", "fs.write", "terminal"],
    authNote: "Uses your Copilot subscription."
  }, {
    name: "Kimi CLI",
    vendor: "Moonshot · native ACP",
    command: "kimi acp",
    state: "missing",
    capabilities: ["fs.read", "fs.write", "terminal"],
    authNote: "Authenticates with a Moonshot key held by the agent."
  }, {
    name: "OpenCode",
    vendor: "opencode.ai · native ACP",
    command: "opencode acp",
    state: "missing",
    capabilities: ["fs.read", "fs.write", "terminal"],
    authNote: "Bring-your-own provider; configured inside OpenCode."
  }, {
    name: "Goose",
    vendor: "Block · native ACP",
    command: "goose acp",
    state: "error",
    capabilities: ["fs.read", "terminal"],
    error: "initialize: protocol version mismatch (agent 0.9, client 1.0)\n  spawn: goose acp — exited 1 after 240ms",
    authNote: "Provider credentials live in Goose's own config."
  }, {
    name: "Custom ACP agent",
    vendor: "any JSON-RPC 2.0 agent on stdio",
    command: "<command> <args…>",
    state: "missing",
    capabilities: [],
    authNote: "Anything in the ACP registry, or your own binary. Binmap speaks the client half of ACP 1.0."
  }]
};
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/binmap-desktop/model-catalog.js", error: String((e && e.message) || e) }); }

__ds_ns.AcpAgentCard = __ds_scope.AcpAgentCard;

__ds_ns.ApiKeyField = __ds_scope.ApiKeyField;

__ds_ns.ModelPicker = __ds_scope.ModelPicker;

__ds_ns.ProviderCard = __ds_scope.ProviderCard;

__ds_ns.ConfidenceBadge = __ds_scope.ConfidenceBadge;

__ds_ns.EvidenceItem = __ds_scope.EvidenceItem;

__ds_ns.FindingCard = __ds_scope.FindingCard;

__ds_ns.GateResult = __ds_scope.GateResult;

__ds_ns.ProposalBlock = __ds_scope.ProposalBlock;

__ds_ns.ProvenanceBadge = __ds_scope.ProvenanceBadge;

__ds_ns.StackFrameList = __ds_scope.StackFrameList;

__ds_ns.TranscriptStep = __ds_scope.TranscriptStep;

__ds_ns.TrustTierControl = __ds_scope.TrustTierControl;

__ds_ns.Flamegraph = __ds_scope.Flamegraph;

__ds_ns.ParetoScatter = __ds_scope.ParetoScatter;

__ds_ns.Treemap = __ds_scope.Treemap;

__ds_ns.CommandPalette = __ds_scope.CommandPalette;

__ds_ns.EmptyState = __ds_scope.EmptyState;

__ds_ns.NavRail = __ds_scope.NavRail;

__ds_ns.StatusBar = __ds_scope.StatusBar;

__ds_ns.TitleBar = __ds_scope.TitleBar;

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.Button = __ds_scope.Button;

__ds_ns.Checkbox = __ds_scope.Checkbox;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Icon = __ds_scope.Icon;

__ds_ns.IconButton = __ds_scope.IconButton;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Panel = __ds_scope.Panel;

__ds_ns.SegmentedControl = __ds_scope.SegmentedControl;

__ds_ns.Select = __ds_scope.Select;

__ds_ns.Switch = __ds_scope.Switch;

__ds_ns.Tabs = __ds_scope.Tabs;

__ds_ns.Tag = __ds_scope.Tag;

__ds_ns.Tooltip = __ds_scope.Tooltip;

__ds_ns.CodeBlock = __ds_scope.CodeBlock;

__ds_ns.DataTable = __ds_scope.DataTable;

__ds_ns.DeltaValue = __ds_scope.DeltaValue;

__ds_ns.MetricStat = __ds_scope.MetricStat;

__ds_ns.ProgressBar = __ds_scope.ProgressBar;

})();
