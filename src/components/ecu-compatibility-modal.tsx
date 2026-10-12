"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { getModalGlassStyle } from "@/lib/modal-glass";
import { ECU_PROGRAM_GROUPS } from "@/lib/ecu/compatibility-reference";

export function EcuCompatibilityModal({ theme, onClose }: {
  theme: "default" | "light" | "oled"; onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const dialogRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previousFocus = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();
    return () => previousFocus?.focus();
  }, []);
  const groups = ECU_PROGRAM_GROUPS.filter(group =>
    `${group.id} ${group.identities} ${group.bankKiB}`.toLowerCase().includes(query.trim().toLowerCase()));
  return <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
    <div ref={dialogRef} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="ecu-compatibility-title"
      className="w-full max-w-4xl max-h-[90vh] overflow-y-auto mx-4 rounded-lg border p-6 space-y-4"
      style={{ ...getModalGlassStyle(theme), color: theme === "light" ? "black" : "white" }}
      onClick={event => event.stopPropagation()} onKeyDown={event => {
        if (event.key === "Escape") { event.stopPropagation(); onClose(); }
        if (event.key === "Tab") {
          const controls = dialogRef.current?.querySelectorAll<HTMLElement>("input, button");
          if (!controls?.length) return;
          const first = controls[0], last = controls[controls.length - 1];
          if (event.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
            event.preventDefault(); last.focus();
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault(); first.focus();
          }
        }
      }}>
      <h2 id="ecu-compatibility-title" className="text-xl font-bold">ECU Compatibility</h2>
      <p className="text-sm">Exact program groups from the inspected 90 hp TDI file collection. Software versions are shown in parentheses. This is a reference to compared files; matching program bytes or bank size does not establish that complete files, calibrations or vehicles are interchangeable.</p>
      <label className="block text-sm">Find an ECU or group
        <input value={query} onChange={event => setQuery(event.target.value)} placeholder="e.g. GN, 4307, P2"
          className="mt-1 block w-full rounded-md border bg-transparent px-3 py-2" />
      </label>
      <div className="overflow-x-auto">
        <table className="w-full text-sm text-left border-collapse">
          <caption className="text-left font-semibold pb-2">Exact program groups</caption>
          <thead><tr className="border-b"><th scope="col" className="p-2">Group</th><th scope="col" className="p-2">ECU identities and software</th><th scope="col" className="p-2">Bank layout</th><th scope="col" className="p-2">Multi-Map Beta</th></tr></thead>
          <tbody>{groups.map(group => <tr key={group.id} className="border-b">
            <th scope="row" className="p-2 align-top">{group.id}</th>
            <td className="p-2">{group.identities}</td>
            <td className="p-2 whitespace-nowrap">{group.bankKiB} KiB</td>
            <td className="p-2">{group.id === "P1" ? "Inspected FD4102 / GN4308 / GQ4307 profiles only" : group.id === "P5" ? "GK KD34: separate experimental profile" : "Not supported"}</td>
          </tr>)}</tbody>
        </table>
        {!groups.length && <p role="status" className="py-3 text-sm">No matching ECU in this reference.</p>}
      </div>
      <p className="text-sm">Bank layout is the observed spacing between calibration banks, not a guarantee of the number of occupied banks. The supported beta layouts use three 64 KiB slots: CB1 [0x4C000, 0x5C000), CB2 [0x5C000, 0x6C000), CB3 [0x6C000, 0x7C000). End addresses are excluded. Firmware validation also checks identities, program fingerprints and bank contents.</p>
      <p className="text-sm">P1 shares the compared main program region; GK belongs to P5 and requires its own strategy. AJ 2829 and AJ 2707 belong to different groups. Other software revisions require separate verification.</p>
      <p className="text-sm">Factory configuration markers in the inspected beta files: CB1 is SG (manual), CB3 is AG (automatic). GN/FD/GQ use bank IDs 2 and 1; GK uses 12 and 11. FD's occupied CB2 has ID 4 and is also marked AG. No verified 4×4 source bank was identified. Converted bank numbers are selector slots and do not identify a transmission.</p>
      <p className="text-sm opacity-80">Reference: offline comparison dated 12 October 2026, 146 unique payloads from 120 archives. Main program comparison: [0x14000, 0x4BFFC) for 64 KiB layouts, [0x14000, 0x57FFC) for 48 KiB layouts. Identification and checksum areas were excluded. These grouping hashes differ from the stricter Multi-Map Beta fingerprints. Site labels do not establish stock provenance.</p>
      <p role="note" className="text-sm text-amber-500">All Multi-Map Beta profiles remain in testing and are used at the user's own risk. This reference does not enable additional profiles or establish flash safety.</p>
      <div className="flex justify-end"><Button onClick={onClose}>Close</Button></div>
    </div>
  </div>;
}
