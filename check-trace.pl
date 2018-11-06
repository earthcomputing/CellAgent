#!/usr/bin/perl -w

use strict;

my $endl = "\n";

my @buffer;

while (<>) {
    push(@buffer, $_);
    next unless /^ *}$/; # block closing line

    my $blk = join('', @buffer);
    next unless $blk =~ m/json!/m; # includes tracking

    my $n = $#buffer;
    my @slice = @buffer[$n - 4..$n];
    my $chunk = join('', @slice);
    print($ARGV, $endl, $chunk);

    @buffer = ();
}
