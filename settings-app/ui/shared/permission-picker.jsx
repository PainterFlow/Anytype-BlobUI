import React from "react";
import { SelectPicker } from "./select-picker.jsx";

export function PermissionPicker({ id, label, options, value, disabled, onChange }) {
  return <SelectPicker label={`${label} for ${id}`} options={options} value={value} onChange={onChange}
    disabled={disabled} rootClassName="permission-picker" menuClassName="permission-picker-menu" fixed />;
}
