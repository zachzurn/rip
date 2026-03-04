param([string]$file)
$port = New-Object System.IO.Ports.SerialPort 'COM5',115200,([System.IO.Ports.Parity]::None),8,([System.IO.Ports.StopBits]::One)
$port.Open()
$bytes = [System.IO.File]::ReadAllBytes($file)
$port.Write($bytes, 0, $bytes.Length)
$port.Close()
Write-Host "Sent $($bytes.Length) bytes to COM5"
