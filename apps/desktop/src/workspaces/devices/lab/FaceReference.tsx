import { AXIS_COLORS } from "./trace";
import { FACES } from "./benchMath";

/** Instructional reference only, not a measured board attitude. */
export function FaceReference({face}:{face:number}) {
  const axis=Math.floor(face/2),sign=face%2?"−":"+";
  const color=AXIS_COLORS[axis];
  return <svg className="studio-face-reference" viewBox="0 0 260 190" role="img" aria-label={`Reference: point ${FACES[face]} vertically upward`}>
    <path d="M20 151 130 100 240 151 130 181Z" fill="var(--surface-2)" stroke="var(--line)"/>
    {[0,1,2,3].map(i=><path key={i} d={`M${20+i*27.5} ${151-i*12.75} l110 30 M${130+i*27.5} ${100+i*12.75} l-110 51`} fill="none" stroke="var(--line)"/>)}
    <polygon points="74,91 130,65 186,91 130,117" fill="var(--surface-3)" stroke="var(--ink-3)"/>
    <polygon points="74,91 130,117 130,155 74,129" fill="var(--surface-1)" stroke="var(--ink-3)"/>
    <polygon points="130,117 186,91 186,129 130,155" fill="var(--surface-2)" stroke="var(--ink-3)"/>
    <polygon points="74,91 130,65 186,91 130,117" fill={color} opacity=".45"/>
    <path d="M130 91V16 M123 24L130 16 137 24" stroke={color} strokeWidth="3" fill="none"/>
    <text x="142" y="26" fill={color} fontSize="14" fontWeight="600">{sign}{"XYZ"[axis]}</text>
    <text x="130" y="185" textAnchor="middle" fontSize="10" fill="var(--ink-3)">Reference orientation · stable surface</text>
  </svg>;
}
