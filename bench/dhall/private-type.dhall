let schema = ../schema/dhall/types.dhall
let t = schema.types
in { ashashi : { users : { email : t.`myConfig.modules.users.email` } }, whiterabbit : { users : { email : t.`myConfig.modules.users.email` }, kanata : { devices : t.`myConfig.modules.kanata.devices` } } }
