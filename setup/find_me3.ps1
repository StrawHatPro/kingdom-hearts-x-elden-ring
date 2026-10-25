# Kingdom Hearts x Elden Ring: asks the player to pick me3.exe (used by er\play.bat when me3 is not found).
# Prints the chosen path, or nothing when the window is cancelled.
Add-Type -AssemblyName System.Windows.Forms
$owner = New-Object System.Windows.Forms.Form -Property @{ TopMost = $true }
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Find me3.exe (Mod Engine 3)'
$dialog.Filter = 'me3.exe|me3.exe|Programs (*.exe)|*.exe'
$dialog.InitialDirectory = [Environment]::GetFolderPath('UserProfile')
if ($dialog.ShowDialog($owner) -eq [System.Windows.Forms.DialogResult]::OK) { $dialog.FileName }
$owner.Dispose()
