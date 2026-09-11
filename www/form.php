<?php
header("Content-Type: text/plain; charset=utf-8");

$name = $_POST["name"] ?? "unknown";
echo "Submitted name: " . $name;

