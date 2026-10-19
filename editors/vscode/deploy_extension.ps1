$srcDir = "D:\dapcode-lang\editors\vscode"
$extBase = Join-Path $env:USERPROFILE ".vscode\extensions"

Get-ChildItem -Path $extBase -Filter "*dapcode-lang*" | ForEach-Object {
    $targetDir = $_.FullName
    Write-Host "Updating $targetDir ..."
    Copy-Item (Join-Path $srcDir "extension.js") $targetDir -Force
    Copy-Item (Join-Path $srcDir "package.json") $targetDir -Force
    Copy-Item (Join-Path $srcDir "language-configuration.json") $targetDir -Force
    if (Test-Path (Join-Path $srcDir "syntaxes")) { Copy-Item (Join-Path $srcDir "syntaxes") $targetDir -Recurse -Force }
    if (Test-Path (Join-Path $srcDir "snippets")) { Copy-Item (Join-Path $srcDir "snippets") $targetDir -Recurse -Force }
    if (Test-Path (Join-Path $srcDir "icons")) { Copy-Item (Join-Path $srcDir "icons") $targetDir -Recurse -Force }
}

$v077Dir = Join-Path $extBase "dapcode-core.dapcode-lang-0.7.7"
if (!(Test-Path $v077Dir)) {
    New-Item -ItemType Directory -Path $v077Dir -Force | Out-Null
    Copy-Item (Join-Path $srcDir "*") $v077Dir -Recurse -Force
}

Write-Host "DapCode Extension successfully updated across all VS Code extension folders!"
