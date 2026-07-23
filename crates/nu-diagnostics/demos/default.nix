{
  pkgs,
  nu-package,
  nu-lint,
}:
let
  inherit (pkgs) lib;

  sceneNames = lib.pipe (builtins.readDir ./.) [
    (lib.filterAttrs (
      file: kind:
      kind == "regular" && lib.hasSuffix ".tape" file && file != "common.tape" && file != "open.tape"
    ))
    builtins.attrNames
    (map (lib.removeSuffix ".tape"))
  ];

  recordScene =
    scene:
    pkgs.stdenvNoCC.mkDerivation {
      name = "nu-diagnostics-recording-${scene}";
      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./common.tape
          ./open.tape
          ./demo_config.nu
          (./. + "/${scene}.tape")
        ];
      };
      nativeBuildInputs = [
        nu-package
        nu-lint
        pkgs.bashInteractive
        pkgs.ffmpeg
        pkgs.vhs
      ];
      FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.jetbrains-mono ]; };
      TZDIR = "${pkgs.tzdata}/share/zoneinfo";
      LC_ALL = "C.UTF-8";
      SHELL = lib.getExe pkgs.bashInteractive;
      VHS_NO_SANDBOX = "true";
      buildPhase = "HOME=$TMPDIR nu ${./record_scene.nu} ${scene}.tape $out";
      dontInstall = true;
    };

  scenes = lib.genAttrs sceneNames recordScene;
in
{
  inherit scenes;
  all = pkgs.symlinkJoin {
    name = "nu-diagnostics-recordings";
    paths = builtins.attrValues scenes;
  };
  publish-recordings = pkgs.writeShellApplication {
    name = "publish-recordings";
    runtimeInputs = [
      nu-package
      pkgs.git
      pkgs.jujutsu
      pkgs.openssh
    ];
    text = ''exec nu ${./publish_recordings.nu} "$@"'';
  };
}
