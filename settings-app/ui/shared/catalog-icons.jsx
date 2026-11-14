import React from "react";

const searchPath = "M8.5 2.5C11.8137 2.5 14.5 5.18629 14.5 8.5C14.5 9.79977 14.0852 11.0019 13.3828 11.9844L17.2227 15.8242C17.6132 16.2147 17.6132 16.8478 17.2227 17.2383C16.8321 17.6288 16.1991 17.6288 15.8086 17.2383L11.9658 13.3955C10.9869 14.0899 9.79171 14.5 8.5 14.5C5.18629 14.5 2.5 11.8137 2.5 8.5C2.5 5.18629 5.18629 2.5 8.5 2.5ZM8.5 4C6.01472 4 4 6.01472 4 8.5C4 10.9853 6.01472 13 8.5 13C10.9853 13 13 10.9853 13 8.5C13 6.01472 10.9853 4 8.5 4Z";
const filterPath = "M3 6C3 5.58579 3.29381 5.25 3.65625 5.25H16.3438C16.7062 5.25 17 5.58579 17 6C17 6.41421 16.7062 6.75 16.3438 6.75H3.65625C3.29381 6.75 3 6.41421 3 6ZM6 10C6 9.58579 6.26863 9.25 6.6 9.25H13.4C13.7314 9.25 14 9.58579 14 10C14 10.4142 13.7314 10.75 13.4 10.75H6.6C6.26863 10.75 6 10.4142 6 10ZM8.75 13.25C8.33579 13.25 8 13.5858 8 14C8 14.4142 8.33579 14.75 8.75 14.75H11.25C11.6642 14.75 12 14.4142 12 14C12 13.5858 11.6642 13.25 11.25 13.25H8.75Z";

export function SearchIcon() {
  return <svg aria-hidden="true" width="20" height="20" viewBox="0 0 20 20" fill="none"><path d={searchPath} fill="currentColor" /></svg>;
}

export function FilterIcon() {
  return <svg aria-hidden="true" viewBox="0 0 20 20" fill="none"><path fillRule="evenodd" clipRule="evenodd" d={filterPath} fill="currentColor" /></svg>;
}

export function ReloadIcon() {
  return <svg aria-hidden="true" viewBox="0 0 20 20"><path d="M16 5.5v4h-4M15.8 9.2a6 6 0 1 0-.7 4" /></svg>;
}

export function GlobeIcon() {
  return <svg aria-hidden="true" focusable="false" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="10" cy="10" r="8.25" />
    <path d="M2.1 10h15.8M10 1.75c2.15 2.25 3.2 4.98 3.2 8.25S12.15 16 10 18.25C7.85 16 6.8 13.27 6.8 10S7.85 4 10 1.75Z" />
  </svg>;
}
