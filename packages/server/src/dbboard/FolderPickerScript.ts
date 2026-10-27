// PowerShell-Skript des Ordnerdialogs. Es nutzt den Explorer-Dialog (IFileOpenDialog mit FOS_PICKFOLDERS, mit
// Adressleiste, Schnellzugriff und Suche) statt des alten Baum-Dialogs der Windows-Forms-Klasse FolderBrowserDialog.
// Der Startordner kommt über die Umgebung (kein Einschleusen in das Skript); Ausgabe: gewählter Pfad oder nichts.
export const FOLDER_PICKER_SCRIPT = String.raw`
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class HarnessFolderPicker
{
    [ComImport, Guid("DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7")]
    private class FileOpenDialogCoClass { }

    [ComImport, Guid("42F85136-DB7E-439C-85F1-E4075D135FC8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IFileDialog
    {
        [PreserveSig] int Show(IntPtr parent);
        void SetFileTypes(uint cFileTypes, IntPtr rgFilterSpec);
        void SetFileTypeIndex(uint iFileType);
        void GetFileTypeIndex(out uint piFileType);
        void Advise(IntPtr pfde, out uint pdwCookie);
        void Unadvise(uint dwCookie);
        void SetOptions(uint fos);
        void GetOptions(out uint pfos);
        void SetDefaultFolder(IShellItem psi);
        void SetFolder(IShellItem psi);
        void GetFolder(out IShellItem ppsi);
        void GetCurrentSelection(out IShellItem ppsi);
        void SetFileName([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetFileName([MarshalAs(UnmanagedType.LPWStr)] out string pszName);
        void SetTitle([MarshalAs(UnmanagedType.LPWStr)] string pszTitle);
        void SetOkButtonLabel([MarshalAs(UnmanagedType.LPWStr)] string pszText);
        void SetFileNameLabel([MarshalAs(UnmanagedType.LPWStr)] string pszLabel);
        void GetResult(out IShellItem ppsi);
        void AddPlace(IShellItem psi, int fdap);
        void SetDefaultExtension([MarshalAs(UnmanagedType.LPWStr)] string pszDefaultExtension);
        void Close(int hr);
        void SetClientGuid(ref Guid guid);
        void ClearClientData();
        void SetFilter(IntPtr pFilter);
    }

    [ComImport, Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItem
    {
        void BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppv);
        void GetParent(out IShellItem ppsi);
        void GetDisplayName(uint sigdnName, [MarshalAs(UnmanagedType.LPWStr)] out string ppszName);
        void GetAttributes(uint sfgaoMask, out uint psfgaoAttribs);
        void Compare(IShellItem psi, uint hint, out int piOrder);
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = false)]
    private static extern void SHCreateItemFromParsingName(string path, IntPtr pbc, ref Guid riid, out IShellItem item);

    private const uint FOS_PICKFOLDERS = 0x20;
    private const uint FOS_FORCEFILESYSTEM = 0x40;
    private const uint FOS_PATHMUSTEXIST = 0x800;
    private const uint SIGDN_FILESYSPATH = 0x80058000;
    private const int ERROR_CANCELLED = unchecked((int)0x800704C7);

    // Zeigt den Dialog; null, wenn abgebrochen
    public static string Pick(string title, string start, IntPtr owner)
    {
        IFileDialog dialog = (IFileDialog)new FileOpenDialogCoClass();
        uint options;
        dialog.GetOptions(out options);
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST);
        dialog.SetTitle(title);
        if (!string.IsNullOrEmpty(start) && System.IO.Directory.Exists(start))
        {
            Guid iid = new Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE");
            IShellItem folder;
            SHCreateItemFromParsingName(start, IntPtr.Zero, ref iid, out folder);
            dialog.SetFolder(folder);
        }
        int hr = dialog.Show(owner);
        if (hr == ERROR_CANCELLED) return null;
        if (hr != 0) Marshal.ThrowExceptionForHR(hr);
        IShellItem result;
        dialog.GetResult(out result);
        string path;
        result.GetDisplayName(SIGDN_FILESYSPATH, out path);
        return path;
    }
}
'@
# Unsichtbares Fenster ganz oben als Besitzer, damit der Dialog vor dem Browser erscheint
$owner = New-Object System.Windows.Forms.Form
$owner.ShowInTaskbar = $false
$owner.Opacity = 0
$owner.TopMost = $true
$owner.Show()
$owner.Activate()
$path = [HarnessFolderPicker]::Pick('Ordner des Repos auf diesem Rechner wählen', $env:HARNESS_PICK_START, $owner.Handle)
if ($path) { [Console]::Out.Write($path) }
`;
