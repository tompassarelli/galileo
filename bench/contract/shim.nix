{
  exportFile,
  firnRoot,
  moduleName ? null,
  original ? null,
  hostSettings ? false,
}:
args@{
  config,
  lib,
  pkgs,
  inputs ? { },
  flakeRoot ? firnRoot,
  ...
}:
let
  export = builtins.fromJSON (builtins.readFile exportFile);
  names = builtins.attrNames export.modules;
  scope = args // {
    inherit builtins inputs flakeRoot;
  };
  decode = value:
    if builtins.isList value then
      map decode value
    else if builtins.isAttrs value then
      if value ? "$ref" then
        lib.attrByPath
          (if builtins.isList value."$ref" then value."$ref" else lib.splitString "." value."$ref")
          (throw "contract: unknown reference ${builtins.toJSON value."$ref"}")
          scope
      else if value ? "$call" then
        builtins.foldl' (function: argument: function (decode argument))
          (decode value."$call".function)
          value."$call".args
      else
        lib.mapAttrs (_: decode) value
    else
      value;
  strip = value:
    value
    // lib.optionalAttrs (value ? myConfig && value.myConfig ? modules) {
      myConfig = value.myConfig // {
        modules = builtins.removeAttrs value.myConfig.modules names;
      };
    };
  base = if original == null then { } else strip (import original args);
  settings = decode export.settings;
in
if moduleName != null then
  decode export.modules.${moduleName}
else
  base
  // lib.optionalAttrs hostSettings {
    imports = (base.imports or [ ]) ++ [
      settings
      {
        myConfig.modules = builtins.listToAttrs (
          map (name: {
            inherit name;
            value.enable = builtins.elem name export.enabled;
          }) names
        );
      }
    ];
  }
