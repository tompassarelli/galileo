let names = ./names.dhall
in List { mapKey : names.Name, mapValue : { tags : List names.Tag, optins : List names.Tag } }
