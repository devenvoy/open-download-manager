param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [Parameter(Mandatory = $true)]
    [ValidateSet("x64", "arm64", "x86")]
    [string]$Expected
)

$resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
$stream = [System.IO.File]::OpenRead($resolved)

try {
    $reader = [System.IO.BinaryReader]::new($stream)
    $stream.Position = 0x3C
    $peOffset = $reader.ReadInt32()
    $stream.Position = $peOffset + 4
    $machine = $reader.ReadUInt16()
}
finally {
    $stream.Dispose()
}

$architectures = @{
    0x014C = "x86"
    0x8664 = "x64"
    0xAA64 = "arm64"
}

$actual = $architectures[[int]$machine]
if ($null -eq $actual) {
    Write-Error "Unknown PE machine value: 0x$('{0:X4}' -f $machine) in $resolved"
    exit 1
}

if ($actual -ne $Expected) {
    Write-Error "Architecture mismatch in $resolved`: expected $Expected, found $actual"
    exit 1
}

Write-Host "OK: $resolved is a native $actual PE binary"
