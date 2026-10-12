// Offline reference for the inspected software versions only; not a flash-compatibility allowlist.
export const ECU_PROGRAM_GROUPS = [
  {
    "id": "P1",
    "identities": "038906012FD (4102); 038906012GN (4308); 038906012GQ (4307)",
    "bankKiB": 64
  },
  {
    "id": "P2",
    "identities": "038906012F (3313, 3331); 038906012FH (3537); 038906012FN (3358); 038906012H (3315, 3332)",
    "bankKiB": 64
  },
  {
    "id": "P3",
    "identities": "038906012AF (2825); 038906012AJ (2829); 038906012B (2802); 038906012C (2803); 038906012CP (2839); 038906012Q (2818)",
    "bankKiB": 48
  },
  {
    "id": "P4",
    "identities": "038906012BD (2507); 038906012M (2528)",
    "bankKiB": 48
  },
  {
    "id": "P5",
    "identities": "038906012GK (KD34)",
    "bankKiB": 64
  },
  {
    "id": "P6",
    "identities": "038906019AB (1071)",
    "bankKiB": 64
  },
  {
    "id": "P7",
    "identities": "038906019LP (1596)",
    "bankKiB": 64
  },
  {
    "id": "P8",
    "identities": "038906012AJ (2707)",
    "bankKiB": 48
  },
  {
    "id": "P9",
    "identities": "038906012S (2512)",
    "bankKiB": 48
  },
  {
    "id": "P10",
    "identities": "038906012BS (2804); 038906012CA (2806)",
    "bankKiB": 48
  },
  {
    "id": "P11",
    "identities": "038906012HA (4922)",
    "bankKiB": 64
  }
] as const;
