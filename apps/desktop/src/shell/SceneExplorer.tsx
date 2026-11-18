import { useEffect, useRef, useState } from "react";
import { Icon, type IconName } from "../ui/Icon";
import type { SceneSelection } from "../lib/sceneSelection";

export type SceneNode = {
  selection: SceneSelection;
  label: string;
  depth?: number;
  detail?: string;
};

export type SceneGroup = {
  label: string;
  icon: IconName;
  nodes: SceneNode[];
  expanded?: boolean;
};

const keyOf = (selection: SceneSelection) =>
  `${selection.kind}:${"index" in selection ? selection.index : selection.id}`;

export function SceneExplorer({ groups, selection, onSelect }: {
  groups: SceneGroup[];
  selection: SceneSelection | null;
  onSelect: (selection: SceneSelection) => void;
}) {
  const [filter, setFilter] = useState("");
  const tree = useRef<HTMLDivElement>(null);
  const selectedRow = useRef<HTMLButtonElement>(null);
  const selectedKey = selection && keyOf(selection);
  const query = filter.trim().toLowerCase();
  const visibleGroups = groups.map((group) => ({
    ...group, visible: group.nodes.filter((node) => node.label.toLowerCase().includes(query)),
  })).filter((group) => !query || group.visible.length > 0);
  useEffect(() => {
    const row = selectedRow.current;
    const list = tree.current;
    if (!row || !list) return;
    const group = row.closest("details");
    if (group) group.open = true;
    if (!row.getClientRects().length) return;
    const bounds = list.getBoundingClientRect();
    const target = row.getBoundingClientRect();
    if (target.top < bounds.top) list.scrollTop += target.top - bounds.top;
    else if (target.bottom > bounds.bottom) list.scrollTop += target.bottom - bounds.bottom;
  }, [selectedKey]);

  return (
    <>
      <div className="scene-search">
        <input type="search" aria-label="Filter scene objects" value={filter}
          placeholder="Filter objects…" onChange={(event) => setFilter(event.target.value)} />
      </div>
      <div className="scene-tree" ref={tree}>
        {visibleGroups.map((group) => (
          <details key={`${group.label}/${query}`} open={!!query || group.expanded ||
            group.visible.some((node) => keyOf(node.selection) === selectedKey)}>
            <summary>
              <Icon name="chevron-down" size={12} /> <Icon name={group.icon} size={14} />
              <span>{group.label}</span><span className="mono scene-count">{group.nodes.length}</span>
            </summary>
            {group.visible.map((node) => {
              const key = keyOf(node.selection);
              const selected = key === selectedKey;
              return (
                <button type="button" key={key} className="scene-node" aria-label={node.label} aria-pressed={selected}
                  ref={selected ? selectedRow : undefined}
                  style={{ paddingInlineStart: 28 + Math.min(node.depth ?? 0, 4) * 10 }}
                  onClick={() => onSelect(node.selection)}>
                  <span>{node.label}</span>
                  {node.detail && <span className="scene-node-detail">{node.detail}</span>}
                </button>
              );
            })}
            {group.nodes.length === 0 && <p className="scene-empty">None available</p>}
          </details>
        ))}
        {visibleGroups.length === 0 && <p className="scene-empty">No matching objects</p>}
      </div>
    </>
  );
}
