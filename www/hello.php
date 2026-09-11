<?php
header("Content-Type: text/plain; charset=utf-8");

$name = $_GET["name"] ?? "World";
echo "Hello, " . $name . "!";

