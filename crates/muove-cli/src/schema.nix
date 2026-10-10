{ flake, host }:
let
  opts = (builtins.getFlake flake).nixosConfigurations.${host}.options;

  describeType = t:
    let
      base = { t = t.name or "?"; };
      tryInner =
        if (t ? nestedTypes) && (t.nestedTypes ? elemType)
        then builtins.tryEval (describeType t.nestedTypes.elemType)
        else { success = false; value = null; };
      withInner =
        if tryInner.success then base // { inner = tryInner.value; } else base;
      tryEnum =
        if (t.name or "") == "enum" && (t ? functor) && (t.functor ? payload) && (t.functor.payload ? values)
        then builtins.tryEval t.functor.payload.values
        else { success = false; value = null; };
      withEnum =
        if tryEnum.success then withInner // { enum = tryEnum.value; } else withInner;
    in withEnum;

  walk = path: o:
    if (o._type or null) == "option"
    then [ ({ name = path; } // describeType (o.type or {})) ]
    else if builtins.isAttrs o
    then builtins.concatLists
      (builtins.attrValues
        (builtins.mapAttrs (n: v: walk (if path == "" then n else "${path}.${n}") v) o))
    else [];
in walk "" opts
