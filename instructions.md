Dans ce fichier on met toutes les instructions pour les différentes branches de développement
pour nous aider a strucutuer notre travail et a suivre les différentes étapes de développement


### Feat21-imple…D-generation 
- to generate unique identifiers we are using the uuid/version 4 (v4) UUIDs ( uuid : unique 128 bit value , stored in 16 octets , formated as 5 groups of hexStrings )
   , rust doesn't know how to stock uuid in the json format so i have to convert it to string before storing it in the json file and when we read it from the json file we have to convert it back to uuid format
- for the date and time we use chrono to provide all functionality needed to do correct operations on dates and times
- Serde is a framework for serializing and deserializing Rust data structures efficiently and generically


### feat3-implem…ection-logic