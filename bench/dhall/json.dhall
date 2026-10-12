let schema = ../schema/dhall/types.dhall
let JSON = schema.types.`home-manager.users.<name>.systemd.user.services.lane-gc.Service.Nice`
let Algebra = \(J : Type) -> { array : List J -> J, bool : Bool -> J, double : Double -> J, integer : Integer -> J, null : J, object : List { mapKey : Text, mapValue : J } -> J, string : Text -> J }
let map = \(A : Type) -> \(B : Type) -> \(f : A -> B) -> \(xs : List A) -> List/fold A xs (List B) (\(x : A) -> \(rest : List B) -> [ f x ] # rest) ([] : List B)
let string = \(v : Text) -> \(J : Type) -> \(j : Algebra J) -> j.string v
let boolean = \(v : Bool) -> \(J : Type) -> \(j : Algebra J) -> j.bool v
let integer = \(v : Integer) -> \(J : Type) -> \(j : Algebra J) -> j.integer v
let natural = \(v : Natural) -> integer (Natural/toInteger v)
let null = \(J : Type) -> \(j : Algebra J) -> j.null
let object = \(xs : List { mapKey : Text, mapValue : JSON }) -> \(J : Type) -> \(j : Algebra J) -> j.object (map { mapKey : Text, mapValue : JSON } { mapKey : Text, mapValue : J } (\(x : { mapKey : Text, mapValue : JSON }) -> { mapKey = x.mapKey, mapValue = x.mapValue J j }) xs)
let array = \(xs : List JSON) -> \(J : Type) -> \(j : Algebra J) -> j.array (map JSON J (\(x : JSON) -> x J j) xs)
let texts = \(xs : List Text) -> array (map Text JSON string xs)
let empty = object ([] : List { mapKey : Text, mapValue : JSON })
let ref = \(parts : List Text) -> object [ { mapKey = "$ref", mapValue = texts parts } ]
let package = \(value : schema.types.`services.kanata.package`) -> merge { Name = \(name : Text) -> object [ { mapKey = "$ref", mapValue = string name } ], Parts = ref } value.`$ref`
let call = \(function : List Text) -> \(args : List JSON) -> object (toMap { `$call` = object (toMap { function = ref function, args = array args }) })
let path = \(value : schema.types.`myConfig.modules.kanata.configFile`) -> merge { String = string, Ref = package, Call = \(value : { `$call` : JSON }) -> object (toMap value) } value
let enable = \(description : Text) -> call [ "lib", "mkEnableOption" ] [ string description ]
let option = \(value : JSON) -> call [ "lib", "mkOption" ] [ value ]
in { JSON = JSON, map = map, string = string, boolean = boolean, integer = integer, natural = natural, null = null, object = object, array = array, texts = texts, empty = empty, ref = ref, package = package, call = call, path = path, enable = enable, option = option }
