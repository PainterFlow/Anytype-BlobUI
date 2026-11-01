function Assert-WorkspaceBuildPath([string]$Path) {
    $full=[IO.Path]::GetFullPath($Path)
    $prefix=$workspaceRoot.TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar
    if(-not $full.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){throw "Build output must stay inside the workspace: $full"}
    $cursor=$full
    while($cursor -and $cursor.Length -ge $workspaceRoot.Length){
        $item=Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue
        if($item -and (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)){throw "Build output cannot use a reparse point: $cursor"}
        $cursor=[IO.Path]::GetDirectoryName($cursor)
    }
}
