#!/usr/bin/perl -w

use strict;

my $endl = "\n";

my @blocks;
my $linger;

my %codex;
my %stalk;

while (<>) {
    # {"a":94366987365694,"f":"multicell::cellagent::CellAgent::initialize::hdedbea7f4e417694","p":"src/cellagent.rs"},
    if (/{("a":\d*),"f":"[^"]*","p":"[^"]*"}/) {
        build_codex($_); #  $codex{$_}++;
        s/{("a":\d*),"f":"[^"]*","p":"[^"]*"}/{$1}/g;
        $stalk{$_}++;
    }

    if (/Traph /) {
        push(@blocks, $linger) if defined $linger;
        $linger = $_;
    }
    else {
        $linger .= $_;
    }
}
push(@blocks, $linger); # if defined $linger;

dump_codex();

my %past;

foreach my $block (@blocks) {
    $block =~ /(Traph .*?)$(.*)/sm;
    my ($key, $body) = ($1, $2);

    unless (defined $key) {
        print('no key: ', $block, $endl);
        next;
    }

    # next unless $key =~ /C:0 /; # tree-id (name)

    $block =~ s/(2 - )/$1\n/; # for readability

    $body =~ s/^backtrace.*$//mg; # drop any stacktraces
    $body =~ s/\n+/\n/g;

    my $was = $past{$key};
    next if defined $was and $body eq $was;

    $past{$key} = $body;
    print($block, $endl);
# print('START', $body, 'END', $endl);
}

exit 0;

sub dump_codex {
    print($endl);
    foreach my $item (sort keys %codex) {
        print('{', $item, '}', $endl);
    }
    print($endl);
    foreach my $item (sort keys %stalk) {
        print($item); # includes endl
    }
    print($endl);
}

# $entry =~ s/},{/}\n{/g;
sub build_codex {
    my ($entry) = @_;
    chomp($entry);
    $entry =~ s/^backtrace: {"frames":\[{//;
    $entry =~ s/\}]}$//;
    my @frames = split('},{', $entry);
    foreach my $item (@frames) { $codex{$item}++; }
}

