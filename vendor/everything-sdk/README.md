# Everything SDK

Le binaire du SDK voidtools n’est pas redistribué dans ce dépôt.

`setup.ps1`, `dev.ps1` et `build.ps1` l’installent automatiquement si nécessaire. Installation manuelle :

```powershell
.\scripts\install-everything-sdk.ps1
```

Le script télécharge `Everything-SDK.zip` depuis le site officiel et copie `Everything64.dll` dans `src-tauri`.
