#!/usr/bin/env bash
set -Eeuo pipefail

workspace=${1:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)}

printf 'Computer: %s\n' "$(hostname)"
printf 'Operating system: %s\n' "$(uname -srm)"

if [[ -f $workspace/anyblob.config.toml ]]; then
    printf '%s\n' 'Project config: present (contents withheld)'
else
    printf '%s\n' 'Project config: not present in this workspace'
fi

mapfile -t matches < <(ps -eo pid=,comm= | awk 'tolower($2) ~ /^(tunnel-client(-runtime)?|blobui|anytype)(\.exe)?$/ {print $1 " " $2}')
if ((${#matches[@]})); then
    printf '%s\n' "${matches[@]}" | while read -r pid name; do
        printf 'Process: %s, PID %s\n' "$name" "$pid"
    done
else
    printf '%s\n' 'Tunnel, BlobUI, and Anytype processes: none found'
fi

mapfile -t tunnel_pids < <(ps -eo pid=,comm= | awk 'tolower($2) ~ /^tunnel-client(-runtime)?(\.exe)?$/ {print $1}')
if ((${#tunnel_pids[@]})); then
    if command -v ss >/dev/null 2>&1; then
        connections=$(ss -Htnp state established 2>/dev/null | awk -v pids="${tunnel_pids[*]}" '
            BEGIN { n=split(pids, a, " "); for (i=1; i<=n; i++) wanted[a[i]]=1 }
            { line=$0; while (match(line, /pid=[0-9]+/)) { pid=substr(line, RSTART+4, RLENGTH-4); if (pid in wanted) { print $4 " " $5; break } line=substr(line, RSTART+RLENGTH) } }
        ')
        if [[ -n $connections ]]; then
            while read -r local_addr remote_addr; do
                remote_host=${remote_addr%:*}
                case $remote_host in
                    127.*|10.*|192.168.*|\[::1\]|\[fc*|\[fd*|\[fe80:*) scope='private/local network' ;;
                    172.*)
                        octet=${remote_host#172.}; octet=${octet%%.*}
                        if [[ $octet =~ ^[0-9]+$ ]] && ((10#$octet >= 16 && 10#$octet <= 31)); then
                            scope='private/local network'
                        else
                            scope='public network'
                        fi
                        ;;
                    *) scope='public network' ;;
                esac
                printf 'Tunnel connection: %s, remote endpoint %s\n' "$scope" "$remote_addr"
            done <<< "$connections"
        else
            printf '%s\n' 'Tunnel client is running, but has no established TCP connection at this moment'
        fi
    else
        printf '%s\n' 'Tunnel client is running, but ss is unavailable, so its connections were not checked'
    fi
fi

printf '%s\n' 'This is a point-in-time check. A public outbound connection alone does not identify the remote service.'
