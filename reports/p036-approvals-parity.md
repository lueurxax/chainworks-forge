# P036 Approvals Parity Checklist

| Feature | Legacy Approvals Tab | P036 Inline Approvals | Status |
| :--- | :--- | :--- | :--- |
| **Approval Listing** | Centralized list | Inline within Run Detail | ✅ Parity |
| **Decision Actions** | Approve / Reject buttons | Inline Approve / Reject | ✅ Parity |
| **Reject Reason** | Optional prompt | Standardized reason | ✅ Parity |
| **Actionability** | Policy-driven disabled state | Policy-driven disabled state | ✅ Parity |
| **Deep-linking** | `cw://approvals` | `cw://runs` + focus on approvals | ✅ Parity |
| **Keyboard Nav** | Cmd+2 | Cmd+1 (within Runs) | ✅ Improved |
| **Empty State** | Generic list view | ContentUnavailableView | ✅ Improved |
| **Error Handling** | Banner | Inline error label | ✅ Parity |
