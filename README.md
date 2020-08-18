This is a unified repository for Earth Computing, intended to painlessly keep work on different subsystems in sync, for as long as it is practical to do so in this way.
Various subsystems are arranged abstractly as follows:

  ^
  |
  |\
  | \
  |  \
  |   \
  |\   \
  | \   \
  |  \   \
  |   \   \
  |    \   \
  |     \   \
  |      |   \
  |      |    \
 top     |     \
         |\     \
         | \     \
         |  \     \
         |   \     \
         |\   \     \
         | \   \     \
         |  \   \     \
         |   \   \     \
         |    \   \     \
         |     |   \     \
         |     |    \     \
         |     |     \     \
         |     |      |     \
         |  e1000e    |      |
         |            |      |
         |          ecnl     |
         |                   |\
       driver                | \
                             |  \
                             |   \
                             |\   \
                             | \   \
                             |  \   \
                             |   \   \
                             |    \   \
                             |     \   \
                             |      |   \
                         userspace  |    \
                                    |     \
                                    |      |
                                cellagent  |
                                           |
                                           |
                                        actix_
                                        server

For each subsystem, there is a branch <subsystem>-master and a branch <subsystem>-staging, such that each 'staging' branch is fed by the 'master' branches of its downstream subsystems and is the sole feed of a 'master' branch of the same subsystem, the intention being that 'staging' branches accept merges from various downstream subsystems in some arbitrary order, and that these are merged into the 'master' branch only when a consistent set of subsystems is in place, so that every commit in a 'master' branch is consistent. The branches for the 'top' subsytem are simply called 'master' and 'staging'.

Guidelines:
Feature branches should be named <subsystem>-<feature>.
Do work in the lowest-level branch for which it is meaningful and appropriate.
The steps below assume that commits are not changed in any remote subsystem branch through a forced push.

To create a feature branch <subsystem>-<feature>:
$ git checkout <subsystem>-<master>
$ git checkout -b <subsystem>-<feature>
To track a remote branch for the feature:
$ git branch -u origin/<subsystem>-<feature>

To include into a feature branch upstream changes to the subsystem branch:
$ git checkout <subsystem>-<feature>
$ git rebase --preserve-merges -i <subsystem>-master
Mark the last (most recent) commit as 'edit'.
Skip any commits from <subsystem>-master and resolve any conflicts in commits from <subsystem>-<feature>.
Test that the feature is still working, updating the feature as necessary.
If the feature can be made to work:
$ git rebase --continue
If not:
$ git rebase --abort

To promote enhancements to a feature branch to the subsystem branch:
$ git checkout <subsystem>-master
Obtain any previous feature enhancements:
$ git pull origin <subsystem>-master
Ensure nothing is broken by the previous feature enhancements.
$ git merge --no-ff <subsystem>-<feature>
Ensure that this feature enhancement is consistent with the rest of the subsystem.
If so:
$ git commit
If not:
$ git merge --abort
