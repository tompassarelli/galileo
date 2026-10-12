{ firnRoot, host ? "whiterabbit" }:
let
  flake = builtins.getFlake "git+file://${firnRoot}?ref=HEAD&rev=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9";
  root = toString flake.outPath;
  nixlib = flake.inputs.nixpkgs.lib;
  names = [ "firefox" "zen-browser" "kanata" "users" "lane-gc" "atuin" "chrome" "godot" "bun" "obsidian" ];
  hostConfig = if host == "whiterabbit" then flake.nixosConfigurations.whiterabbit.config else flake.darwinConfigurations.ashashi.config;
  symbol = value: value // { __toString = _: "@AF6@${builtins.toJSON (encode value)}@/AF6@"; };
  ref = path: symbol { "$ref" = path; };
  call = path: args: symbol { "$call" = { function = { "$ref" = path; }; inherit args; }; };
  omitted = { __af6Omitted = true; };
  lib = nixlib // {
    mkIf = condition: value: if condition then value else omitted;
    mkOption = value: call [ "lib" "mkOption" ] [ value ];
    mkEnableOption = value: call [ "lib" "mkEnableOption" ] [ value ];
    mkDefault = value: call [ "lib" "mkDefault" ] [ value ];
    mkForce = value: call [ "lib" "mkForce" ] [ value ];
    makeBinPath = value: call [ "lib" "makeBinPath" ] [ value ];
    types = (nixlib.mapAttrs (name: _: ref [ "lib" "types" name ]) nixlib.types) // {
      listOf = value: call [ "lib" "types" "listOf" ] [ value ];
      nullOr = value: call [ "lib" "types" "nullOr" ] [ value ];
    };
  };
  pkgs = builtins.listToAttrs (map (name: { inherit name; value = ref [ "pkgs" name ]; }) [ "bashInteractive" "bun" "git" "openssh" "gh" "coreutils" "google-chrome" "obsidian" ]) // {
    unstable = { kanata = ref [ "pkgs" "unstable" "kanata" ]; godot_4 = ref [ "pkgs" "unstable" "godot_4" ]; };
    stdenv.hostPlatform.system = ref [ "pkgs" "stdenv" "hostPlatform" "system" ];
  };
  inputs.zen-browser.packages.${toString pkgs.stdenv.hostPlatform.system}.default = call [ "builtins" "getAttr" ] [
    "default"
    (call [ "builtins" "getAttr" ] [ pkgs.stdenv.hostPlatform.system (ref [ "inputs" "zen-browser" "packages" ]) ])
  ];
  args = { config = hostConfig; inherit lib pkgs inputs; flakeRoot = ref [ "flakeRoot" ]; };
  importSource = path: builtins.scopedImport {
    builtins = builtins // { readFile = value: call [ "builtins" "readFile" ] [ value ]; };
  } path args;
  importModule = path:
    let value = importSource path;
    in value // nixlib.optionalAttrs (value ? imports) { imports = map importModule value.imports; };
  encodeString = value:
    let
      marked = builtins.replaceStrings [ root ] [ (toString (ref [ "flakeRoot" ])) ] value;
      parts = map (part: if builtins.isList part then builtins.fromJSON (builtins.head part) else part) (builtins.split "@AF6@([^@]*)@/AF6@" marked);
    in if builtins.length parts == 1 then value else {
      "$call" = { function = { "$ref" = [ "lib" "concatStrings" ]; }; args = [ (builtins.filter (part: part != "") parts) ]; };
    };
  encode = value:
    if builtins.isFunction value then encode (value { config = { }; })
    else if builtins.isList value then map encode (builtins.filter (item: !(builtins.isAttrs item && item ? __af6Omitted)) value)
    else if builtins.isAttrs value then
      nixlib.mapAttrs (_: encode) (nixlib.filterAttrs (_: item: !(builtins.isAttrs item && item ? __af6Omitted)) (builtins.removeAttrs value [ "__toString" ]))
    else if builtins.isString value || builtins.isPath value then encodeString (toString value)
    else value;
  selected = value: nixlib.filterAttrs (name: _: builtins.elem name names) (value.myConfig.modules or { });
  moduleNames = builtins.filter (name: builtins.hasAttr "enable" ((hostConfig.myConfig.modules or { }).${name} or { })) names;
  settings = nixlib.recursiveUpdate (selected (importSource (root + "/hosts/${host}/configuration.nix"))) (selected (importSource (root + "/hosts/${host}/_generated-enables.nix")));
in
encode {
  enabled = builtins.filter (name: hostConfig.myConfig.modules.${name}.enable or false) names;
  settings.myConfig.modules = nixlib.mapAttrs (_: value: builtins.removeAttrs value [ "enable" ]) settings;
  modules = builtins.listToAttrs (map (name: { inherit name; value = importModule (root + "/modules/${name}/default.nix"); }) moduleNames);
}
