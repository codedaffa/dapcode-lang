$srcDir = "D:\dapcode-lang\editors\vscode"
$extBases = @(
    (Join-Path $env:USERPROFILE ".vscode\extensions"),
    (Join-Path $env:USERPROFILE ".antigravity-ide\extensions")
)

foreach ($extBase in $extBases) {
    if (Test-Path $extBase) {
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

        $pkgJson = Get-Content (Join-Path $srcDir "package.json") -Raw | ConvertFrom-Json
        $verDir = Join-Path $extBase "dapcode-core.dapcode-lang-$($pkgJson.version)"
        if (!(Test-Path $verDir)) {
            New-Item -ItemType Directory -Path $verDir -Force | Out-Null
        }
        Copy-Item (Join-Path $srcDir "*") $verDir -Recurse -Force
    }
}

Write-Host "DapCode Extension successfully updated across all extension folders!"
