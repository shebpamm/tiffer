{ pkgs, lib, config, inputs, ... }:

{
  languages.rust.enable = true;
  languages.rust.channel = "nightly";
}
