import { useState } from "react";
import type { DeviceStatus } from "@contracts";
import { humanizeId } from "../../lib/format";
import { Icon } from "../../ui/Icon";

interface SegmentRow {
  id: string;
  index: number;
  depth: number;
}
interface Props {
  rows: SegmentRow[];
  cameras: DeviceStatus[];
  selectedSegment: number;
  onSelectSegment: (index: number) => void;
}

export function SceneExplorer({ rows, cameras, selectedSegment, onSelectSegment }: Props) {
  const [filter, setFilter] = useState("");
  const query = filter.trim().toLowerCase();
  const segments = rows.filter((row) => humanizeId(row.id).toLowerCase().includes(query));
  const visibleCameras = cameras.filter((camera) =>
    camera.descriptor.id.toLowerCase().includes(query),
  );
  return (
    <section className="scene-explorer" aria-label="Scene explorer">
      <div className="panel-heading">
        SCENE EXPLORER <span className="mono">{rows.length + cameras.length}</span>
      </div>
      <div className="scene-search">
        <input type="search" aria-label="Filter scene objects" value={filter}
          placeholder="Filter objects…" onChange={(event) => setFilter(event.target.value)} />
      </div>
      <div className="scene-tree">
        <details open>
          <summary><Icon name="layers" size={13} /> Athlete / Skeleton</summary>
          <div role="group" aria-label="Body segments">
            {segments.map((row) => (
              <button type="button" key={row.id}
                className={`scene-node ${selectedSegment === row.index ? "selected" : ""}`}
                style={{ paddingLeft: `${10 + Math.min(row.depth, 4) * 12}px` }}
                aria-pressed={selectedSegment === row.index}
                onClick={() => onSelectSegment(row.index)}>
                <Icon name="target" size={12} />
                <span>{humanizeId(row.id)}</span>
              </button>
            ))}
            {segments.length === 0 && <p className="scene-empty">No matching segments</p>}
          </div>
        </details>
        <details open>
          <summary><Icon name="camera" size={13} /> Cameras ({cameras.length})</summary>
          {visibleCameras.map((camera) => (
            <a key={camera.descriptor.id} className="scene-node" href="#/cameras">
              <Icon name="video" size={12} />
              <span>{camera.descriptor.id}</span>
            </a>
          ))}
        </details>
      </div>
    </section>
  );
}
