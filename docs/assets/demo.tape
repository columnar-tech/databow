Output docs/assets/demo.gif

Require databow

Set Shell "bash"
Set FontSize 18
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"

Type "databow --profile warehouse"
Sleep 500ms
Enter
Sleep 1s

Type "SELECT * FROM penguins LIMIT 5;"
Sleep 500ms
Enter
Sleep 2s

Type "SELECT species, island, COUNT(*)"
Enter
Type "FROM penguins"
Enter
Type "GROUP BY species, island"
Enter
Type "ORDER BY species, island;"
Sleep 500ms
Enter
Sleep 2s

Type "SELECT species, AVG(body_mass_g) AS avg_body_mass"
Enter
Type "FROM penguins"
Enter
Type "GROUP BY species"
Enter
Type "ORDER BY avg_body_mass DESC;"
Sleep 500ms
Enter
Sleep 4s
