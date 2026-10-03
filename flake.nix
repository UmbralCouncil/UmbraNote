{
  description = "Umbra Note TypeScript desktop application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version = "0.0.1";
        app = pkgs.stdenvNoCC.mkDerivation {
          pname = "umbra-note";
          inherit version;
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let name = baseNameOf path;
              in !(
                type == "directory"
                && builtins.elem name [
                  ".git"
                  "node_modules"
                  "target"
                  "dist"
                  "dist-electron"
                ]
              );
          };
          npmDeps = pkgs.importNpmLock.buildNodeModules {
            npmRoot = ./.;
            nodejs = pkgs.nodejs_24;
            derivationArgs = {
              # Electron itself comes from nixpkgs below. Its npm package is
              # needed only for TypeScript declarations and must never attempt
              # a network download in the sandboxed dependency build.
              env.ELECTRON_SKIP_BINARY_DOWNLOAD = "1";
            };
          };
          nativeBuildInputs = [
            pkgs.nodejs_24
            pkgs.importNpmLock.hooks.linkNodeModulesHook
          ];
          npmRoot = ".";
          env.ELECTRON_SKIP_BINARY_DOWNLOAD = "1";
          buildPhase = ''
            runHook preBuild
            npm run build
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            appRoot=$out/lib/umbra-note/app
            mkdir -p "$appRoot" $out/bin $out/share/applications \
              $out/share/icons/hicolor/512x512/apps
            cp -R dist dist-electron assets "$appRoot/"
            install -Dm444 package.json "$appRoot/package.json"
            # nixpkgs exposes the unpacked Electron distribution below
            # libexec/electron. Dereference libexec because it is a symlink to
            # electron-unwrapped and the release archive must contain the
            # runtime rather than a dangling Nix-store link.
            cp -RL ${pkgs.electron}/libexec/electron "$out/lib/umbra-note/electron"
            cat > $out/bin/umbra-note <<'SH'
            #!/bin/sh
            root="$(CDPATH= cd -- "$(dirname -- "$0")/../lib/umbra-note" && pwd)"
            exec "$root/electron/electron" "$root/app" "$@"
            SH
            chmod 0555 $out/bin/umbra-note
            install -Dm444 assets/note.png \
              $out/share/icons/hicolor/512x512/apps/umbra-note.png
            cat > $out/share/applications/umbra-note.desktop <<'DESKTOP'
            [Desktop Entry]
            Name=Umbra Note
            Comment=Local-first Markdown notebook
            Exec=umbra-note
            Icon=umbra-note
            Terminal=false
            Type=Application
            Categories=Office;TextEditor;
            MimeType=text/markdown;
            DESKTOP
            runHook postInstall
          '';
          meta = {
            description = "Local-first Markdown notebook with live preview";
            license = pkgs.lib.licenses.gpl3Plus;
            mainProgram = "umbra-note";
            platforms = [ "x86_64-linux" "aarch64-linux" ];
          };
        };
      in {
        packages.default = app;
        packages.release-bundle = pkgs.runCommand "umbra-note-${version}-${system}.tar.zst" {
          nativeBuildInputs = [ pkgs.gnutar pkgs.zstd ];
        } ''
          mkdir bundle
          cp -R ${app}/bin ${app}/lib ${app}/share bundle/
          if find bundle -type f \( -name '*.ts' -o -name '*.tsx' -o \
              -name '*.map' -o -name package-lock.json \) | grep -q .; then
            echo "source or development files leaked into the release bundle" >&2
            exit 1
          fi
          tar --sort=name --mtime='@1' --owner=0 --group=0 --numeric-owner \
            -C bundle -cf - . | zstd -19 -T0 -o $out
        '';
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ nodejs_24 electron ];
          ELECTRON_SKIP_BINARY_DOWNLOAD = "1";
        };
      });
}
