import { useCallback, useEffect, useState } from "react";
import {
  getMicrophonePermissionState,
  getScreenRecordingPermissionState,
  getAccessibilityPermissionState,
  type MicPermissionState,
  type ScreenRecordingPermissionState,
  type AccessibilityPermissionState,
} from "../lib/tauri";

/** The three permission states, and whether the setup screen is shown. */
export function usePermissions() {
  const [micPermission, setMicPermission] =
    useState<MicPermissionState>("unknown");
  const [screenRecordingPermission, setScreenRecordingPermission] =
    useState<ScreenRecordingPermissionState>("unknown");
  const [accessibilityPermission, setAccessibilityPermission] =
    useState<AccessibilityPermissionState>("unknown");
  // The first-run screen is dismissed for this launch, not forever: the
  // recorder's banner keeps carrying whatever is still missing.
  const [permissionSetupSkipped, setPermissionSetupSkipped] = useState(false);

  const refreshPermissions = useCallback(async () => {
    const [mic, screen, accessibility] = await Promise.all([
      getMicrophonePermissionState(),
      getScreenRecordingPermissionState(),
      getAccessibilityPermissionState(),
    ]);
    setMicPermission(mic);
    setScreenRecordingPermission(screen);
    setAccessibilityPermission(accessibility);
    // Returned as well as stored: a caller that needs the answer now cannot
    // read state it set in the same tick.
    return { mic, screen, accessibility };
  }, []);

  // Screen Recording and Accessibility are granted in System Settings, not in
  // the dialog, so the app is not told when it happens. Poll while the setup
  // screen is up, and only while it is up, so returning from System Settings
  // updates the rows instead of leaving them stale. Stops as soon as
  // everything is granted, or the screen is dismissed.
  const permissionsAllGranted =
    micPermission === "granted" &&
    screenRecordingPermission === "granted" &&
    accessibilityPermission === "granted";
  const permissionSetupVisible =
    !permissionsAllGranted && !permissionSetupSkipped;

  useEffect(() => {
    if (!permissionSetupVisible) return;
    const id = setInterval(() => void refreshPermissions(), 1500);
    return () => clearInterval(id);
  }, [permissionSetupVisible, refreshPermissions]);

  // Reopen the permission setup rather than firing prompts from here.
  //
  // The banner used to call a batch request, which could not help in the one
  // state it was ever shown in: macOS raises a permission dialog only while a
  // permission is undetermined, and a banner about a *denied* permission is by
  // definition past that. Worse, firing three prompts at once meant only one
  // dialog could be presented and the rest were auto-denied. The setup screen
  // carries the per-permission action that does work.
  const handleOpenPermissionSetup = useCallback(() => {
    setPermissionSetupSkipped(false);
  }, []);

  return {
    micPermission,
    screenRecordingPermission,
    accessibilityPermission,
    refreshPermissions,
    permissionSetupVisible,
    skipPermissionSetup: () => setPermissionSetupSkipped(true),
    handleOpenPermissionSetup,
  };
}
